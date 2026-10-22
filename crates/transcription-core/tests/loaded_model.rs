//! 真实模型验证：任务结束后模型应驻留内存，`loaded_model_id()` 返回该模型。
//!
//! 需要环境变量 `VOICE_TO_TEXT_TEST_MODEL_DIR` 指向已安装的 sherpa-onnx 模型目录。
//! 运行：`VOICE_TO_TEXT_TEST_MODEL_DIR=... cargo test -p voice-to-text-core --test loaded_model -- --ignored`

use std::{
    fs,
    path::{Path, PathBuf},
};

use voice_to_text_core::{inspect_model_directory, loaded_model_id, transcribe_file};

fn write_silence_wav(path: &Path, ms: u32) {
    let sample_rate = 16_000u32;
    let samples = sample_rate * ms / 1000;
    let data_len = samples * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    fs::write(path, bytes).unwrap();
}

#[test]
#[ignore = "needs VOICE_TO_TEXT_TEST_MODEL_DIR pointing at an installed model"]
fn model_stays_resident_after_a_job() {
    let dir = std::env::var("VOICE_TO_TEXT_TEST_MODEL_DIR")
        .expect("set VOICE_TO_TEXT_TEST_MODEL_DIR to an installed model directory");
    let model = inspect_model_directory(PathBuf::from(dir).as_path()).unwrap();
    assert_eq!(loaded_model_id(), None, "fresh process has nothing loaded");

    let root = std::env::temp_dir().join(format!("lt-resident-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let wav = root.join("silence.wav");
    write_silence_wav(&wav, 1_200);

    transcribe_file(&wav, &model, |_| {}).unwrap();
    assert_eq!(
        loaded_model_id(),
        Some(model.id.clone()),
        "after a job the model must still be resident for the green dot"
    );
}
