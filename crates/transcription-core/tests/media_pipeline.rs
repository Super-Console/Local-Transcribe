use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use voice_to_text_core::{decode_audio_file, inspect_model_directory, ModelFamily};

fn scratch(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "voice-to-text-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_pcm_wav(path: &std::path::Path, sample_rate: u32, duration_seconds: u32) {
    let samples = sample_rate * duration_seconds;
    let data_size = samples * 2;
    let mut bytes = Vec::with_capacity((44 + data_size) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for index in 0..samples {
        let phase = std::f32::consts::TAU * 440.0 * index as f32 / sample_rate as f32;
        let value = (phase.sin() * i16::MAX as f32 * 0.2) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn decodes_and_resamples_audio_without_changing_the_source_file() {
    let root = scratch("decode");
    let path = root.join("tone.wav");
    write_pcm_wav(&path, 8_000, 1);
    let original = fs::read(&path).unwrap();

    let decoded = decode_audio_file(&path, 16_000).unwrap();

    assert_eq!(decoded.sample_rate, 16_000);
    assert_eq!(decoded.samples.len(), 16_000);
    assert_eq!(decoded.duration_ms, 1_000);
    assert!(decoded.samples.iter().all(|sample| sample.is_finite()));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn rejects_data_that_is_not_a_supported_audio_or_video_file() {
    let root = scratch("invalid-media");
    let path = root.join("broken.wav");
    fs::write(&path, b"not wave data").unwrap();

    assert!(decode_audio_file(&path, 16_000).is_err());
}

#[test]
fn discovers_a_paraformer_model_only_when_its_required_assets_exist() {
    let root = scratch("paraformer-model");
    let model_dir = root.join("sherpa-onnx-paraformer-zh");
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.int8.onnx"), b"test-model-placeholder").unwrap();
    fs::write(model_dir.join("tokens.txt"), b"<blk>\n").unwrap();

    let model = inspect_model_directory(&model_dir).unwrap();

    assert_eq!(model.family, ModelFamily::Paraformer);
    assert!(model.model_path.ends_with("model.int8.onnx"));
    assert!(model.tokens_path.ends_with("tokens.txt"));
}

#[test]
fn refuses_model_directories_missing_tokens_instead_of_claiming_ready() {
    let root = scratch("incomplete-model");
    let model_dir = root.join("sherpa-onnx-sense-voice-zh-en");
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.int8.onnx"), b"test-model-placeholder").unwrap();

    assert!(inspect_model_directory(&model_dir).is_err());
}
