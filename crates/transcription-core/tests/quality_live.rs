use std::{env, fs, path::PathBuf};

use voice_to_text_core::{
    inspect_model_directory, transcribe_file_streaming_with_options, TranscriptionOptions,
};

const PUNCTUATION: &str = "，。？！、；：,.?!;:";

fn count_punctuation(text: &str) -> usize {
    text.chars()
        .filter(|value| PUNCTUATION.contains(*value))
        .count()
}

fn max_repeat_run(text: &str) -> usize {
    let chars: Vec<char> = text
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect();
    if chars.is_empty() {
        return 0;
    }
    let mut best = 1usize;
    let mut run = 1usize;
    for pair in chars.windows(2) {
        if pair[0] == pair[1] {
            run += 1;
            best = best.max(run);
        } else {
            run = 1;
        }
    }
    best
}

/// 手工质量测量：转写一段真实音频并打印可量化指标（分段数、字数、标点数、
/// 最长连续重复），可选把识别文本写到 `LT_TEST_OUT`。
///
/// 运行方式（默认忽略）：
/// `VOICE_TO_TEXT_TEST_MODEL_DIR=... VOICE_TO_TEXT_TEST_MEDIA=... \
///  cargo test -p voice-to-text-core --test quality_live -- --ignored --nocapture`
#[test]
#[ignore = "manual quality measurement; needs a model, audio and optional punctuation model"]
fn measure_quality() {
    let model_dir =
        PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MODEL_DIR").expect("model directory"));
    let media = PathBuf::from(env::var_os("VOICE_TO_TEXT_TEST_MEDIA").expect("media path"));
    let punctuation_model = env::var_os("VOICE_TO_TEXT_TEST_PUNCT_MODEL").map(PathBuf::from);
    let out = env::var_os("LT_TEST_OUT").map(PathBuf::from);

    let model = inspect_model_directory(model_dir).expect("compatible model directory");
    let mut segments = Vec::new();
    let transcript = transcribe_file_streaming_with_options(
        &media,
        &model,
        &TranscriptionOptions { punctuation_model },
        || false,
        |_| {},
        |update| segments.push(update.segment),
    )
    .expect("offline inference succeeds");

    assert!(!transcript.text.trim().is_empty());
    println!("segments={}", segments.len());
    println!("chars={}", transcript.text.chars().count());
    println!("punctuation={}", count_punctuation(&transcript.text));
    println!("max_repeat_run={}", max_repeat_run(&transcript.text));
    println!("duration_ms={:?}", transcript.duration_ms);
    if let Some(path) = out {
        fs::write(&path, &transcript.text).expect("write transcript");
    }
}
