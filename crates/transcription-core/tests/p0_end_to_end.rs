use std::{env, fs, path::PathBuf, time::Instant};

use voice_to_text_core::{inspect_model_directory, transcribe_file};

/// P0 gate: video/audio decode -> PCM -> real model load -> transcription ->
/// stable segments -> JSON export, while the source media stays byte-identical.
///
/// Requires an approved local model directory and a local media fixture:
///   VOICE_TO_TEXT_TEST_MODEL_DIR  absolute path to a validated model layout
///   VOICE_TO_TEXT_TEST_MEDIA      absolute path to the audio/video fixture
///   VOICE_TO_TEXT_TEST_JSON_OUT   optional path for the exported JSON artifact
#[test]
#[ignore = "P0 gate: requires an approved model directory and a local media fixture"]
fn decodes_media_and_exports_stable_json_without_touching_the_source() {
    let model_dir =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("set model directory"));
    let media_path =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("set media fixture path"));
    let json_out = env::var_os("VOICE_TO_TEXT_TEST_JSON_OUT").map(PathBuf::from);

    let model = inspect_model_directory(model_dir).expect("compatible model directory");
    let original_bytes = fs::read(&media_path).expect("read source before inference");
    let original_modified = fs::metadata(&media_path)
        .expect("read source metadata before inference")
        .modified()
        .expect("read source modification time before inference");

    let started = Instant::now();
    let transcript =
        transcribe_file(&media_path, &model, |_| {}).expect("offline inference succeeds");
    let elapsed = started.elapsed();

    // Real text is mandatory: an empty transcript is a failure, not a pass.
    assert!(
        !transcript.text.trim().is_empty(),
        "the speech engine should return text for this spoken sample"
    );
    assert_eq!(transcript.engine, "sherpa-onnx/1.13.8");
    assert_eq!(transcript.schema_version, "1");
    let duration_ms = transcript.duration_ms.expect("decoded audio duration");
    assert!(duration_ms > 0);

    // Stable segment structure with usable time metadata.
    let segments = transcript.segments.as_ref().expect("segment list");
    assert!(!segments.is_empty(), "at least one stable segment");
    assert_eq!(segments.first().unwrap().start_ms, 0);
    let mut previous_end = 0u64;
    for segment in segments {
        assert!(segment.start_ms <= segment.end_ms);
        assert!(segment.start_ms >= previous_end);
        previous_end = segment.end_ms;
        assert!(!segment.text.trim().is_empty());
    }
    assert!(previous_end <= duration_ms);

    // JSON round-trip through a standard parser keeps every field.
    let json = transcript.to_json().expect("serialize JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse exported JSON");
    assert_eq!(parsed["schemaVersion"], "1");
    assert_eq!(parsed["text"], transcript.text);
    assert_eq!(parsed["durationMs"], duration_ms);
    assert!(parsed["segments"].as_array().is_some());
    assert_eq!(parsed["segments"].as_array().unwrap().len(), segments.len());

    // TXT contains only the recognized text.
    assert_eq!(transcript.to_txt(), transcript.text);

    if let Some(out) = json_out {
        fs::write(&out, &json).expect("write exported JSON");
        let reread = fs::read_to_string(&out).expect("read exported JSON back");
        assert_eq!(
            reread, json,
            "exported file must match the serialized result"
        );
        println!("JSON artifact: {}", out.display());
    }

    // The source media must stay byte-identical and keep its modification time.
    assert_eq!(
        fs::read(&media_path).expect("read source after inference"),
        original_bytes,
        "source media must not be modified"
    );
    assert_eq!(
        fs::metadata(&media_path)
            .expect("read source metadata after inference")
            .modified()
            .expect("read source modification time after inference"),
        original_modified
    );

    let audio_seconds = duration_ms as f64 / 1000.0;
    let wall_seconds = elapsed.as_secs_f64();
    println!(
        "source={} model={} audio={:.2}s wall={:.2}s rtf={:.3} segments={}",
        transcript.source_file_name,
        transcript.model_id,
        audio_seconds,
        wall_seconds,
        wall_seconds / audio_seconds,
        segments.len()
    );
    println!("Recognized text: {}", transcript.text);
}
