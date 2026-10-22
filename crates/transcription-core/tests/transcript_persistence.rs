use voice_to_text_core::{Transcript, TranscriptSegment};

#[test]
fn saved_json_transcripts_can_be_reloaded_without_losing_fields() {
    let transcript = Transcript {
        schema_version: "1".to_owned(),
        job_id: "test-job-1".to_owned(),
        source_file_name: "sample.wav".to_owned(),
        model_id: "paraformer-local".to_owned(),
        model_version: None,
        engine: "sherpa-onnx/1.13.8".to_owned(),
        language: Some("zh".to_owned()),
        duration_ms: Some(1_250),
        text: "本地转写结果".to_owned(),
        segments: Some(vec![TranscriptSegment {
            start_ms: 0,
            end_ms: 1_250,
            text: "本地转写结果".to_owned(),
        }]),
    };

    let json = transcript.to_json().unwrap();
    let reloaded: Transcript = serde_json::from_str(&json).unwrap();

    assert_eq!(reloaded, transcript);
}
