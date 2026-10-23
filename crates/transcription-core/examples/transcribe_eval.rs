//! 开发用批量评测：给定一个模型目录和一组音频，逐条转写并输出文本。
//!
//! 用法（不参与自动化测试）：
//! `cargo run --release -p voice-to-text-core --example transcribe_eval -- \
//!     <model_dir> <media_dir> <out_dir>`
//!
//! 输出每个文件的时长、转写耗时、字符数，并把文本写入
//! `<out_dir>/<文件名>.txt`，便于与标准答案做 CER 对照。

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use voice_to_text_core::{
    inspect_model_directory, transcribe_file_streaming_with_options, TranscriptionOptions,
};

fn main() {
    let mut args = env::args_os().skip(1);
    let model_dir = PathBuf::from(args.next().expect("model directory"));
    let media_dir = PathBuf::from(args.next().expect("media directory"));
    let out_dir = PathBuf::from(args.next().expect("output directory"));
    fs::create_dir_all(&out_dir).expect("create output directory");

    let model = inspect_model_directory(&model_dir).expect("compatible model directory");
    println!("model\t{}\t{:?}", model.id, model.family);

    let mut media = fs::read_dir(&media_dir)
        .expect("media directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| {
                    matches!(ext.to_ascii_lowercase().as_str(), "wav" | "m4a" | "mp3")
                })
        })
        .collect::<Vec<_>>();
    media.sort();

    for path in media {
        let started = Instant::now();
        let transcript = transcribe_file_streaming_with_options(
            &path,
            &model,
            &TranscriptionOptions::default(),
            || false,
            |_| {},
            |_| {},
        )
        .expect("offline inference succeeds");
        let elapsed = started.elapsed().as_millis();
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        fs::write(out_dir.join(format!("{stem}.txt")), &transcript.text).expect("write transcript");
        println!(
            "{}\t{}\t{}\t{}",
            file_label(&path),
            transcript.duration_ms.unwrap_or(0),
            elapsed,
            transcript.text.chars().count()
        );
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned()
}
