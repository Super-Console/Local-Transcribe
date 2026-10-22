use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use voice_to_text_core::{ModelFamily, ModelLibrary};

fn scratch(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "voice-to-text-library-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_paraformer_model(directory: &std::path::Path) {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("model.int8.onnx"), b"model-bytes").unwrap();
    fs::write(directory.join("tokens.txt"), b"<blk>\nsil\n").unwrap();
}

#[test]
fn import_copies_the_model_and_leaves_the_source_untouched() {
    let root = scratch("import");
    let source = root.join("sherpa-onnx-paraformer-zh");
    write_paraformer_model(&source);
    let source_bytes = fs::read(source.join("model.int8.onnx")).unwrap();
    let library = ModelLibrary::new(root.join("app-data"));

    let imported = library.import(&source).expect("import succeeds");

    assert_eq!(imported.model.family, ModelFamily::Paraformer);
    assert!(imported.model.directory.starts_with(library.directory()));
    assert!(imported.model.directory.join("model.int8.onnx").is_file());
    assert_eq!(
        imported.bytes,
        b"model-bytes".len() as u64 + b"<blk>\nsil\n".len() as u64
    );
    // 源目录保持完整，未被移动或改写。
    assert_eq!(
        fs::read(source.join("model.int8.onnx")).unwrap(),
        source_bytes
    );
    assert_eq!(library.list().unwrap().len(), 1);
}

#[test]
fn import_rejects_an_incompatible_folder_without_leaving_partial_copies() {
    let root = scratch("reject");
    let source = root.join("not-a-model");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("weights.bin"), b"junk").unwrap();
    let library = ModelLibrary::new(root.join("app-data"));

    let error = library
        .import(&source)
        .expect_err("invalid layout is rejected");
    assert!(
        error.contains("Whisper") || error.contains("模型"),
        "{error}"
    );

    let leftovers: Vec<_> = fs::read_dir(library.directory())
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(".import-"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "partial copies must not remain: {leftovers:?}"
    );
    assert!(library.list().unwrap().is_empty());
}

#[test]
fn importing_the_same_model_twice_is_refused_but_keeps_the_first_copy() {
    let root = scratch("duplicate");
    let source = root.join("sherpa-onnx-paraformer-zh");
    write_paraformer_model(&source);
    let library = ModelLibrary::new(root.join("app-data"));

    let first = library.import(&source).expect("first import");
    let error = library
        .import(&source)
        .expect_err("second import is refused");
    assert!(error.contains("已在库中"), "{error}");
    assert_eq!(library.list().unwrap().len(), 1);
    assert!(first.model.directory.is_dir());
}

#[test]
fn deleting_a_model_moves_it_to_the_trash_and_keeps_the_download() {
    let root = scratch("delete");
    let source = root.join("sherpa-onnx-paraformer-zh");
    write_paraformer_model(&source);
    let library = ModelLibrary::new(root.join("app-data"));
    let imported = library.import(&source).expect("import");
    let trash = root.join("trash");

    let moved = library
        .delete_to(&imported.model.id, &trash)
        .expect("delete moves to trash");

    assert!(moved.starts_with(&trash));
    assert!(
        moved.join("model.int8.onnx").is_file(),
        "copy lands in the trash"
    );
    assert!(!imported.model.directory.exists(), "library copy removed");
    assert!(
        source.join("model.int8.onnx").is_file(),
        "download untouched"
    );
    assert!(library.list().unwrap().is_empty());
}

#[test]
fn library_listing_skips_folders_that_are_not_valid_models() {
    let root = scratch("listing");
    let library = ModelLibrary::new(root.join("app-data"));
    write_paraformer_model(&library.directory().join("sherpa-onnx-paraformer-zh"));
    fs::create_dir_all(library.directory().join("broken")).unwrap();
    fs::write(library.directory().join("broken/weights.bin"), b"junk").unwrap();

    let models = library.list().unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].model.id.contains("paraformer"));
}
