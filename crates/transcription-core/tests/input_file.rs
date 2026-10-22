use std::{fs, path::PathBuf};

use voice_to_text_core::InputFile;

fn project_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")
}

#[test]
fn inspects_a_regular_file_without_modifying_it() {
    let path = project_manifest();
    let before = fs::metadata(&path).expect("project manifest should be readable");

    let input = InputFile::inspect(&path).expect("regular files should be inspectable");

    assert_eq!(input.file_name, "Cargo.toml");
    assert_eq!(input.size_bytes, before.len());
    assert_eq!(input.path, path);
    assert_eq!(fs::metadata(&path).unwrap().len(), before.len());
}

#[test]
fn rejects_a_directory_as_an_input_file() {
    let path = project_manifest().parent().unwrap().to_path_buf();

    let error = InputFile::inspect(&path).unwrap_err();

    assert_eq!(
        error.to_string(),
        "The selected input is not a regular file."
    );
}

#[test]
fn reports_a_missing_input_without_echoing_its_path() {
    let path = std::env::temp_dir().join(format!(
        "voice-to-text-missing-input-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    assert!(!path.exists(), "test path must not already exist");

    let error = InputFile::inspect(&path).unwrap_err();

    assert_eq!(error.to_string(), "The selected input cannot be read.");
    assert!(!path.exists(), "inspection must not create a missing input");
}
