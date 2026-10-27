use std::{fs::File, io::ErrorKind, path::Path};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CodecParameters, DecoderOptions},
    errors::Error as SymphoniaError,
    formats::FormatReader,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_ms: u64,
}

/// 流式解码的结果：交给调用方的样本数与媒体时长。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedAudioMeta {
    pub duration_ms: u64,
    pub sample_count: u64,
}

/// Decode the first audio track without writing to or otherwise changing the source.
pub fn decode_audio_file(
    path: impl AsRef<Path>,
    target_sample_rate: u32,
) -> Result<DecodedAudio, String> {
    let mut samples = Vec::new();
    let meta = decode_audio_file_streaming(path, target_sample_rate, |block| {
        samples.extend_from_slice(block);
        Ok(())
    })?;
    Ok(DecodedAudio {
        samples,
        sample_rate: target_sample_rate,
        duration_ms: meta.duration_ms,
    })
}

/// 只探测第一条音轨的时长（毫秒），不解码音频样本。
///
/// 容器没有给出帧数时返回 `None`；调用方据此决定能否显示“共几段”。
pub fn probe_audio_duration_ms(path: impl AsRef<Path>) -> Result<Option<u64>, String> {
    let format = open_probe(path.as_ref())?;
    let selection = select_audio(format.as_ref())?;
    let (Some(frames), Some(rate)) = (
        selection.codec_params.n_frames,
        selection.codec_params.sample_rate,
    ) else {
        return Ok(None);
    };
    if frames == 0 || rate == 0 {
        return Ok(None);
    }
    Ok(Some(
        ((frames as u128 * 1_000 + rate as u128 / 2) / rate as u128) as u64,
    ))
}

/// 边解码边把目标采样率的单声道样本交给 `on_samples`。
///
/// 整条音轨不驻留内存：两小时的视频若整段解码会占用约 1.9 GB，Windows 上分配失败
/// 会直接终止进程（release 构建 `panic = "abort"`），所以长媒体必须流式处理。
pub fn decode_audio_file_streaming(
    path: impl AsRef<Path>,
    target_sample_rate: u32,
    mut on_samples: impl FnMut(&[f32]) -> Result<(), String>,
) -> Result<DecodedAudioMeta, String> {
    if target_sample_rate == 0 {
        return Err("The target sample rate must be greater than zero.".to_owned());
    }

    let mut format = open_probe(path.as_ref())?;
    let selection = select_audio(format.as_ref())?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&selection.codec_params, &DecoderOptions::default())
        .map_err(|error| format!("The audio codec is not supported: {error}"))?;

    let mut resampler: Option<StreamingResampler> = None;
    let mut source_rate = None;
    let mut mono: Vec<f32> = Vec::new();
    let mut resampled: Vec<f32> = Vec::new();
    let mut total_in: u64 = 0;
    let mut delivered: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => return Err(format!("Could not read the media stream: {error}")),
        };
        if packet.track_id() != selection.track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(format!("Could not decode the audio track: {error}")),
        };
        let rate = decoded.spec().rate;
        if source_rate.is_some_and(|existing| existing != rate) {
            return Err(
                "The audio sample rate changes within this media; this file is not supported yet."
                    .to_owned(),
            );
        }
        source_rate = Some(rate);
        let channels = decoded.spec().channels.count();
        if channels == 0 {
            return Err("The selected audio track has no channels.".to_owned());
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        mono.clear();
        for frame in buffer.samples().chunks_exact(channels) {
            mono.push(frame.iter().sum::<f32>() / channels as f32);
        }
        total_in += mono.len() as u64;

        let resampler =
            resampler.get_or_insert_with(|| StreamingResampler::new(rate, target_sample_rate));
        resampled.clear();
        resampler.push(&mono, &mut resampled);
        if !resampled.is_empty() {
            on_samples(&resampled)?;
            delivered += resampled.len() as u64;
        }
    }

    let rate = source_rate.ok_or_else(|| "No decodable audio samples were found.".to_owned())?;
    if total_in == 0 {
        return Err("The selected media contains no audio samples.".to_owned());
    }
    if let Some(resampler) = resampler.as_mut() {
        resampled.clear();
        resampler.finish(&mut resampled);
        if !resampled.is_empty() {
            on_samples(&resampled)?;
            delivered += resampled.len() as u64;
        }
    }
    let duration_ms = ((total_in as u128 * 1_000 + rate as u128 / 2) / rate as u128) as u64;
    Ok(DecodedAudioMeta {
        duration_ms,
        sample_count: delivered,
    })
}

fn open_probe(path: &Path) -> Result<Box<dyn FormatReader>, String> {
    let file =
        File::open(path).map_err(|error| format!("Cannot open the selected media: {error}"))?;
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let source = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &symphonia::core::formats::FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map(|probed| probed.format)
        .map_err(|error| format!("The selected file is not a supported media container: {error}"))
}

struct AudioSelection {
    track_id: u32,
    codec_params: CodecParameters,
}

fn select_audio(format: &dyn FormatReader) -> Result<AudioSelection, String> {
    let track = format
        .default_track()
        .filter(|track| track.codec_params.sample_rate.is_some())
        .or_else(|| {
            format
                .tracks()
                .iter()
                .find(|track| track.codec_params.sample_rate.is_some())
        })
        .ok_or_else(|| "No audio track was found in the selected media.".to_owned())?;
    Ok(AudioSelection {
        track_id: track.id,
        codec_params: track.codec_params.clone(),
    })
}

/// 有状态的线性重采样器：输出与整段 `resample_linear` 完全一致，但只保留需要的窗口。
struct StreamingResampler {
    source_rate: u32,
    target_rate: u32,
    /// 尚未消费完的源样本。
    pending: Vec<f32>,
    /// `pending[0]` 对应的全局源样本下标。
    pending_start: u64,
    /// 下一个要产出的输出样本下标。
    next_output: u64,
}

impl StreamingResampler {
    fn new(source_rate: u32, target_rate: u32) -> Self {
        Self {
            source_rate,
            target_rate,
            pending: Vec::new(),
            pending_start: 0,
            next_output: 0,
        }
    }

    fn push(&mut self, block: &[f32], out: &mut Vec<f32>) {
        if self.source_rate == self.target_rate {
            out.extend_from_slice(block);
            return;
        }
        self.pending.extend_from_slice(block);
        while let Some(sample) = self.next_sample() {
            out.push(sample);
        }
    }

    fn next_sample(&mut self) -> Option<f32> {
        let position = self.next_output as f64 * self.source_rate as f64 / self.target_rate as f64;
        let left = position.floor() as u64;
        let index = left.checked_sub(self.pending_start)? as usize;
        if index + 1 >= self.pending.len() {
            return None;
        }
        let left_sample = self.pending[index];
        let right_sample = self.pending[index + 1];
        let fraction = (position - left as f64) as f32;
        if index > 0 {
            self.pending.drain(..index);
            self.pending_start = left;
        }
        self.next_output += 1;
        Some(left_sample + (right_sample - left_sample) * fraction)
    }

    /// 收尾：补出剩余的样本；最后一段没有后继样本时按整段解码的规则取最后一个样本。
    fn finish(&mut self, out: &mut Vec<f32>) {
        if self.source_rate == self.target_rate {
            return;
        }
        let total_in = self.pending_start + self.pending.len() as u64;
        let expected = ((total_in as u128 * self.target_rate as u128
            + self.source_rate as u128 / 2)
            / self.source_rate as u128) as u64;
        while self.next_output < expected {
            let position =
                self.next_output as f64 * self.source_rate as f64 / self.target_rate as f64;
            let left = position.floor() as u64;
            let index = left.saturating_sub(self.pending_start) as usize;
            let Some(&left_sample) = self.pending.get(index) else {
                break;
            };
            let right_sample = self.pending.get(index + 1).copied().unwrap_or(left_sample);
            let fraction = (position - left as f64) as f32;
            out.push(left_sample + (right_sample - left_sample) * fraction);
            self.next_output += 1;
        }
    }
}
