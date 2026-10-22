use std::{env, fs, path::PathBuf};

use voice_to_text_core::{inspect_model_directory, transcribe_file, transcribe_file_streaming};

/// Real-model check: segments are reported in order while the file is being
/// transcribed, and they match the final transcript exactly.
#[test]
#[ignore = "requires an installed model and a local test recording"]
fn streams_stable_segments_in_order_during_transcription() {
    let model_dir =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("set model directory"));
    let media_path =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("set test media path"));
    let model = inspect_model_directory(model_dir).expect("compatible model directory");

    let mut streamed = Vec::new();
    let mut progress_calls = 0usize;
    let transcript = transcribe_file_streaming(
        &media_path,
        &model,
        || false,
        |_| progress_calls += 1,
        |update| {
            assert_eq!(
                update.index,
                streamed.len(),
                "segments must arrive in order"
            );
            assert!(update.total_chunks >= 1);
            assert!(!update.segment.text.trim().is_empty());
            streamed.push(update.segment);
        },
    )
    .expect("offline inference succeeds");

    let final_segments = transcript.segments.expect("final segments");
    assert!(!streamed.is_empty(), "streaming must report real text");
    assert_eq!(
        streamed, final_segments,
        "streamed text equals the final result"
    );
    assert!(progress_calls >= 1);
    assert_eq!(transcript.text.split('\n').count(), final_segments.len());
    println!("streamed {} segments", streamed.len());
}

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
