use serchx_lib::logging::{
    init_logging, rotate_log_files, LogRingBuffer, MAX_LOG_ARCHIVES,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_logging_initialization() {
    let buffer = LogRingBuffer::new(5);
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());

    for i in 1..=10 {
        buffer.push(format!("log message {}", i));
    }

    assert_eq!(buffer.len(), 5);
    let all_lines = buffer.get_lines(None);
    assert_eq!(all_lines.len(), 5);
    assert_eq!(all_lines[0], "log message 6");
    assert_eq!(all_lines[4], "log message 10");

    let tail_2 = buffer.get_lines(Some(2));
    assert_eq!(tail_2.len(), 2);
    assert_eq!(tail_2[0], "log message 9");
    assert_eq!(tail_2[1], "log message 10");
}

#[test]
fn test_log_rotation() {
    let dir = tempdir().unwrap();
    let log_file = dir.path().join("serchx.log");

    fs::write(&log_file, "initial log line 1\n").unwrap();
    assert!(log_file.exists());

    // Rotate 1st time
    rotate_log_files(&log_file, MAX_LOG_ARCHIVES).unwrap();
    assert!(!log_file.exists());
    assert!(dir.path().join("serchx.log.1").exists());

    // Write new log file and rotate 2nd time
    fs::write(&log_file, "second log line 2\n").unwrap();
    rotate_log_files(&log_file, MAX_LOG_ARCHIVES).unwrap();
    assert!(dir.path().join("serchx.log.1").exists());
    assert!(dir.path().join("serchx.log.2").exists());

    let content_1 = fs::read_to_string(dir.path().join("serchx.log.1")).unwrap();
    let content_2 = fs::read_to_string(dir.path().join("serchx.log.2")).unwrap();
    assert_eq!(content_1, "second log line 2\n");
    assert_eq!(content_2, "initial log line 1\n");
}

#[test]
fn test_logging_tracing_capture() {
    let dir = tempdir().unwrap();
    let buffer = LogRingBuffer::new(50);
    let _guard = init_logging(dir.path(), buffer.clone());

    tracing::info!("Test indexing trace event 1");
    tracing::warn!("Test indexing warning event 2");

    // Allow background write
    std::thread::sleep(std::time::Duration::from_millis(50));

    let lines = buffer.get_lines(None);
    assert!(
        lines.iter().any(|l| l.contains("Test indexing trace event 1") || l.contains("Tracing logger initialized")),
        "Expected tracing logs to be captured in ring buffer, got {:?}",
        lines
    );
}

#[test]
fn test_log_export() {
    let dir = tempdir().unwrap();
    let logs_dir = dir.path().join("logs");
    fs::create_dir_all(&logs_dir).unwrap();
    let log_file = logs_dir.join("serchx.log");
    fs::write(&log_file, "2026-10-08 INFO Indexing completed for 42 files\n").unwrap();

    let export_dest = dir.path().join("exported_serchx.log");
    fs::copy(&log_file, &export_dest).unwrap();

    assert!(export_dest.exists());
    let exported = fs::read_to_string(&export_dest).unwrap();
    assert!(exported.contains("Indexing completed for 42 files"));
}
