use std::{fs::File, io::ErrorKind, path::Path};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::FormatOptions,
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

/// Decode the first audio track without writing to or otherwise changing the source.
pub fn decode_audio_file(
    path: impl AsRef<Path>,
    target_sample_rate: u32,
) -> Result<DecodedAudio, String> {
    if target_sample_rate == 0 {
        return Err("The target sample rate must be greater than zero.".to_owned());
    }

    let path = path.as_ref();
    let file =
        File::open(path).map_err(|error| format!("Cannot open the selected media: {error}"))?;
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let source = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| {
            format!("The selected file is not a supported media container: {error}")
        })?;
    let mut format = probed.format;

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
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|error| format!("The audio codec is not supported: {error}"))?;

    let mut source_rate = None;
    let mut mono_samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => return Err(format!("Could not read the media stream: {error}")),
        };
        if packet.track_id() != track_id {
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
        for frame in buffer.samples().chunks_exact(channels) {
            mono_samples.push(frame.iter().sum::<f32>() / channels as f32);
        }
    }

    let source_rate =
        source_rate.ok_or_else(|| "No decodable audio samples were found.".to_owned())?;
    if mono_samples.is_empty() {
        return Err("The selected media contains no audio samples.".to_owned());
    }
    let duration_ms = ((mono_samples.len() as u128 * 1_000 + source_rate as u128 / 2)
        / source_rate as u128) as u64;
    let samples = resample_linear(&mono_samples, source_rate, target_sample_rate);

    Ok(DecodedAudio {
        samples,
        sample_rate: target_sample_rate,
        duration_ms,
    })
}

fn resample_linear(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source_rate == target_rate {
        return input.to_vec();
    }
    let target_len = ((input.len() as u128 * target_rate as u128 + source_rate as u128 / 2)
        / source_rate as u128) as usize;
    let mut output = Vec::with_capacity(target_len);
    for index in 0..target_len {
        let position = index as f64 * source_rate as f64 / target_rate as f64;
        let left = position.floor() as usize;
        let right = (left + 1).min(input.len() - 1);
        let fraction = (position - left as f64) as f32;
        output.push(input[left] + (input[right] - input[left]) * fraction);
    }
    output
}
