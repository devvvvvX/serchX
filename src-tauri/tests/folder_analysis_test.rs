use serchx_lib::commands::onboarding::analyze_folder_path;
use serchx_lib::db::DatabaseManager;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_analyze_folder_path_breakdown_and_eta() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // Documents
    let doc_dir = root.join("docs");
    fs::create_dir_all(&doc_dir).unwrap();
    File::create(doc_dir.join("report.pdf"))
        .unwrap()
        .write_all(b"PDF content")
        .unwrap();
    File::create(doc_dir.join("notes.txt"))
        .unwrap()
        .write_all(b"Plain text")
        .unwrap();

    // Images
    let img_dir = root.join("images");
    fs::create_dir_all(&img_dir).unwrap();
    File::create(img_dir.join("photo.png"))
        .unwrap()
        .write_all(b"PNG image bytes")
        .unwrap();

    // Code
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    File::create(src_dir.join("main.rs"))
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();
    File::create(src_dir.join("app.py"))
        .unwrap()
        .write_all(b"print('hello')")
        .unwrap();
    File::create(src_dir.join("index.js"))
        .unwrap()
        .write_all(b"console.log(1)")
        .unwrap();

    // Data
    File::create(root.join("config.json"))
        .unwrap()
        .write_all(b"{}")
        .unwrap();

    // Ignored / Blacklisted
    File::create(root.join("archive.zip"))
        .unwrap()
        .write_all(b"PK...")
        .unwrap();
    File::create(root.join(".hidden.txt"))
        .unwrap()
        .write_all(b"hidden")
        .unwrap();

    // Pruned directory
    let nm = root.join("node_modules");
    fs::create_dir_all(&nm).unwrap();
    File::create(nm.join("dep.js"))
        .unwrap()
        .write_all(b"dep")
        .unwrap();

    let db = DatabaseManager::new_in_memory().unwrap();
    let report = analyze_folder_path(&root.to_string_lossy(), true, &db).expect("analyze folder");

    assert_eq!(report.folder_path, root.to_string_lossy());
    assert_eq!(report.documents_count, 2, "2 documents (pdf, txt)");
    assert_eq!(report.images_count, 1, "1 image (png)");
    assert_eq!(report.code_count, 3, "3 code files (rs, py, js)");
    assert_eq!(report.data_count, 1, "1 data file (json)");
    assert_eq!(report.total_indexable_files, 7, "Total 7 indexable files");
    assert!(report.total_indexable_bytes > 0, "Non-zero indexable bytes");
    assert!(
        report.ignored_files_count >= 2,
        "archive.zip and .hidden.txt ignored"
    );
    assert!(
        report.estimated_time_seconds >= 1,
        "ETA should be >= 1s for indexable files"
    );
}
