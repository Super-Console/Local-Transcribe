use std::{env, fs, path::PathBuf};

use voice_to_text_core::{inspect_model_directory, transcribe_file};

#[test]
#[ignore = "requires an installed sherpa-onnx model and a short local test recording"]
fn transcribes_a_local_recording_without_changing_it() {
    let model_dir =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("set model directory"));
    let media_path =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("set test media path"));
    let model = inspect_model_directory(model_dir).expect("compatible model directory");
    let original_bytes = fs::read(&media_path).expect("read source before inference");

    let transcript =
        transcribe_file(&media_path, &model, |_| {}).expect("offline inference succeeds");

    assert!(
        !transcript.text.trim().is_empty(),
        "the speech engine should return text for this spoken sample"
    );
    assert_eq!(transcript.engine, "sherpa-onnx/1.13.8");
    assert!(transcript.duration_ms.unwrap_or_default() > 0);
    assert_eq!(
        fs::read(&media_path).expect("read source after inference"),
        original_bytes
    );
    println!("Recognized text: {}", transcript.text);
}
