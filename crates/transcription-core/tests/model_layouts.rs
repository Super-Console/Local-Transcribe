use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use voice_to_text_core::{inspect_model_directory, ModelFamily};

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

fn touch(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

const WHISPER_TOKENS: &str = "<|startoftranscript|>\n<|en|>\n<|transcribe|>\n";

#[test]
fn recognizes_a_whisper_int8_export_layout() {
    let root = scratch("whisper-layout");
    let model_dir = root.join("sherpa-onnx-whisper-turbo");
    touch(
        &model_dir.join("turbo-encoder.int8.onnx"),
        b"encoder-placeholder",
    );
    touch(
        &model_dir.join("turbo-decoder.int8.onnx"),
        b"decoder-placeholder",
    );
    touch(
        &model_dir.join("turbo-tokens.txt"),
        WHISPER_TOKENS.as_bytes(),
    );

    let model = inspect_model_directory(&model_dir).expect("whisper layout is recognized");

    assert_eq!(model.family, ModelFamily::Whisper);
    assert!(model.id.starts_with("whisper-"));
    match &model.files {
        voice_to_text_core::ModelFiles::Whisper {
            encoder,
            decoder,
            tokens,
        } => {
            assert!(encoder.ends_with("turbo-encoder.int8.onnx"));
            assert!(decoder.ends_with("turbo-decoder.int8.onnx"));
            assert!(tokens.ends_with("turbo-tokens.txt"));
        }
        other => panic!("expected whisper files, got {other:?}"),
    }
}

#[test]
fn recognizes_a_qwen3_asr_int8_export_layout() {
    let root = scratch("qwen3-layout");
    let model_dir = root.join("sherpa-onnx-qwen3-asr-0.6B-int8");
    touch(
        &model_dir.join("conv_frontend.onnx"),
        b"frontend-placeholder",
    );
    touch(&model_dir.join("encoder.int8.onnx"), b"encoder-placeholder");
    touch(&model_dir.join("decoder.int8.onnx"), b"decoder-placeholder");
    touch(&model_dir.join("tokenizer/vocab.json"), b"{}");
    touch(&model_dir.join("tokenizer/merges.txt"), b"#version: 0.2\n");
    touch(
        &model_dir.join("tokenizer/tokenizer_config.json"),
        b"{\"model_max_length\": 32768}",
    );

    let model = inspect_model_directory(&model_dir).expect("qwen3 layout is recognized");

    assert_eq!(model.family, ModelFamily::Qwen3Asr);
    assert!(model.id.starts_with("qwen3-asr-"));
    match &model.files {
        voice_to_text_core::ModelFiles::Qwen3Asr {
            conv_frontend,
            encoder,
            decoder,
            tokenizer_dir,
        } => {
            assert!(conv_frontend.ends_with("conv_frontend.onnx"));
            assert!(encoder.ends_with("encoder.int8.onnx"));
            assert!(decoder.ends_with("decoder.int8.onnx"));
            assert!(tokenizer_dir.ends_with("tokenizer"));
        }
        other => panic!("expected qwen3 files, got {other:?}"),
    }
}

#[test]
fn refuses_transducer_layouts_instead_of_mislabeling_them_as_whisper() {
    let root = scratch("transducer-layout");
    let model_dir = root.join("sherpa-onnx-zipformer-en");
    touch(&model_dir.join("encoder.int8.onnx"), b"encoder-placeholder");
    touch(&model_dir.join("decoder.int8.onnx"), b"decoder-placeholder");
    touch(&model_dir.join("joiner.int8.onnx"), b"joiner-placeholder");
    touch(&model_dir.join("tokens.txt"), b"<blk>\nsil\n");

    let error = inspect_model_directory(&model_dir).expect_err("transducer is unsupported");
    assert!(
        error.to_lowercase().contains("transducer"),
        "error should name the rejected layout: {error}"
    );
}

#[test]
fn refuses_qwen3_layouts_missing_required_tokenizer_files() {
    let root = scratch("qwen3-incomplete");
    let model_dir = root.join("sherpa-onnx-qwen3-asr-0.6B-int8");
    touch(
        &model_dir.join("conv_frontend.onnx"),
        b"frontend-placeholder",
    );
    touch(&model_dir.join("encoder.int8.onnx"), b"encoder-placeholder");
    touch(&model_dir.join("decoder.int8.onnx"), b"decoder-placeholder");

    let error = inspect_model_directory(&model_dir).expect_err("tokenizer is required");
    assert!(
        error.contains("vocab.json"),
        "error should name the missing tokenizer file: {error}"
    );
}

#[test]
fn refuses_unknown_layouts_with_a_reason_that_lists_supported_formats() {
    let root = scratch("unknown-layout");
    let model_dir = root.join("some-random-model");
    touch(&model_dir.join("weights.bin"), b"placeholder");

    let error = inspect_model_directory(&model_dir).expect_err("unknown layout");
    assert!(
        error.contains("Whisper") && error.contains("Qwen3"),
        "error should list supported layouts: {error}"
    );
}

#[test]
fn recognizes_a_large_base64_whisper_vocabulary_without_special_tokens() {
    let root = scratch("whisper-vocab");
    let model_dir = root.join("sherpa-onnx-whisper-large-v3");
    touch(&model_dir.join("large-v3-encoder.int8.onnx"), b"encoder");
    touch(&model_dir.join("large-v3-decoder.int8.onnx"), b"decoder");
    let mut vocabulary = String::with_capacity(40_001 * 8);
    for id in 0..40_001 {
        vocabulary.push_str("Ig== ");
        vocabulary.push_str(&id.to_string());
        vocabulary.push('\n');
    }
    touch(
        &model_dir.join("large-v3-tokens.txt"),
        vocabulary.as_bytes(),
    );

    let model = inspect_model_directory(&model_dir).expect("whisper-scale vocabulary is accepted");
    assert_eq!(model.family, ModelFamily::Whisper);
}

#[test]
fn recognizes_a_vocabulary_with_the_whisper_marker() {
    let root = scratch("whisper-marker");
    let model_dir = root.join("whisper-custom");
    touch(&model_dir.join("encoder.onnx"), b"encoder");
    touch(&model_dir.join("decoder.onnx"), b"decoder");
    touch(
        &model_dir.join("tokens.txt"),
        b"PHxzdGFydG9mdHJhbnNjcmlwdHw+ 50257\n",
    );

    let model = inspect_model_directory(&model_dir).expect("marker is accepted");
    assert_eq!(model.family, ModelFamily::Whisper);
}

#[test]
fn recognizes_a_fire_red_asr2_aed_layout() {
    let root = scratch("fire-red-layout");
    let model_dir = root.join("sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26");
    touch(&model_dir.join("encoder.int8.onnx"), b"encoder-placeholder");
    touch(&model_dir.join("decoder.int8.onnx"), b"decoder-placeholder");
    // FireRedASR uses a char-level vocabulary, not a Whisper vocabulary.
    touch(
        &model_dir.join("tokens.txt"),
        "<pad> 0\n<unk> 1\n的 2\n".as_bytes(),
    );

    let model = inspect_model_directory(&model_dir).expect("fire-red-asr layout is recognized");

    assert_eq!(model.family, ModelFamily::FireRedAsr);
    assert!(model.id.starts_with("fire-red-asr-"));
    match &model.files {
        voice_to_text_core::ModelFiles::FireRedAsr {
            encoder,
            decoder,
            tokens,
        } => {
            assert!(encoder.ends_with("encoder.int8.onnx"));
            assert!(decoder.ends_with("decoder.int8.onnx"));
            assert!(tokens.ends_with("tokens.txt"));
        }
        other => panic!("expected fire-red-asr files, got {other:?}"),
    }
}

#[test]
fn refuses_small_encoder_decoder_vocabularies_that_are_not_whisper() {
    let root = scratch("not-whisper");
    let model_dir = root.join("some-encoder-decoder");
    touch(&model_dir.join("encoder.onnx"), b"encoder");
    touch(&model_dir.join("decoder.onnx"), b"decoder");
    touch(&model_dir.join("tokens.txt"), b"<blk> 0\nsil 1\n");

    let error = inspect_model_directory(&model_dir).expect_err("small vocab is rejected");
    assert!(
        error.contains("Whisper"),
        "reason should name the layout: {error}"
    );
}
