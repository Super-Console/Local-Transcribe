use serde_json::Value;
use voice_to_text_core::{Transcript, TranscriptSegment};

fn sample_transcript(segments: Option<Vec<TranscriptSegment>>) -> Transcript {
    Transcript {
        schema_version: "1.0".to_owned(),
        job_id: "job-test-01".to_owned(),
        source_file_name: "lecture.m4a".to_owned(),
        model_id: "qwen3-asr-0.6b-int8".to_owned(),
        model_version: Some("test-revision".to_owned()),
        engine: "sherpa-onnx".to_owned(),
        language: Some("zh".to_owned()),
        duration_ms: Some(12_500),
        text: "第一句。 Second line.".to_owned(),
        segments,
    }
}

#[test]
fn json_export_contains_versioned_metadata_and_complete_text() {
    let encoded = sample_transcript(None).to_json().unwrap();
    let value: Value = serde_json::from_str(&encoded).unwrap();

    assert_eq!(value["schemaVersion"], "1.0");
    assert_eq!(value["jobId"], "job-test-01");
    assert_eq!(value["sourceFileName"], "lecture.m4a");
    assert_eq!(value["modelId"], "qwen3-asr-0.6b-int8");
    assert_eq!(value["engine"], "sherpa-onnx");
    assert_eq!(value["language"], "zh");
    assert_eq!(value["durationMs"], 12_500);
    assert_eq!(value["text"], "第一句。 Second line.");
    assert!(value.get("segments").is_none());
}

#[test]
fn json_export_preserves_optional_timestamps_and_escapes_text() {
    let mut transcript = sample_transcript(Some(vec![TranscriptSegment {
        start_ms: 250,
        end_ms: 1_750,
        text: "他说：\"你好\"\\再见".to_owned(),
    }]));
    transcript.text = "他说：\"你好\"\\再见".to_owned();

    let encoded = transcript.to_json().unwrap();
    let value: Value = serde_json::from_str(&encoded).unwrap();

    assert_eq!(value["text"], "他说：\"你好\"\\再见");
    assert_eq!(value["segments"][0]["startMs"], 250);
    assert_eq!(value["segments"][0]["endMs"], 1_750);
    assert_eq!(value["segments"][0]["text"], "他说：\"你好\"\\再见");
}

#[test]
fn txt_export_contains_only_the_transcript_text() {
    let transcript = sample_transcript(None);

    assert_eq!(transcript.to_txt(), "第一句。 Second line.");
}
