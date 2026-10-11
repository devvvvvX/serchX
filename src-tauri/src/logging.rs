use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub const DEFAULT_MAX_LOG_ENTRIES: usize = 250;
pub const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10MB
pub const MAX_LOG_ARCHIVES: usize = 3;

#[derive(Clone)]
pub struct LogRingBuffer {
    buffer: Arc<RwLock<VecDeque<String>>>,
    max_capacity: usize,
}

impl LogRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            max_capacity: capacity,
        }
    }

    pub fn push(&self, line: String) {
        if let Ok(mut lock) = self.buffer.write() {
            if lock.len() >= self.max_capacity {
                lock.pop_front();
            }
            lock.push_back(line);
        }
    }

    pub fn get_lines(&self, tail_lines: Option<usize>) -> Vec<String> {
        if let Ok(lock) = self.buffer.read() {
            let n = tail_lines.unwrap_or(lock.len());
            let start = lock.len().saturating_sub(n);
            lock.iter().skip(start).cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.read().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn raw_buffer(&self) -> Arc<RwLock<VecDeque<String>>> {
        self.buffer.clone()
    }
}

impl Default for LogRingBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_LOG_ENTRIES)
    }
}

#[derive(Clone)]
struct RingBufferWriter {
    buffer: LogRingBuffer,
}

impl Write for RingBufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            for line in s.lines() {
                if !line.is_empty() {
                    self.buffer.push(line.to_string());
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RingBufferWriter {
    type Writer = RingBufferWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

pub fn rotate_log_files(log_path: &Path, max_archives: usize) -> std::io::Result<()> {
    if !log_path.exists() {
        return Ok(());
    }

    let parent = log_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = log_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("serchx.log");

    for i in (1..=max_archives).rev() {
        let old_archive = parent.join(format!("{}.{}", stem, i));
        if i == max_archives {
            if old_archive.exists() {
                let _ = fs::remove_file(&old_archive);
            }
        } else {
            let next_archive = parent.join(format!("{}.{}", stem, i + 1));
            if old_archive.exists() {
                let _ = fs::rename(&old_archive, &next_archive);
            }
        }
    }

    let archive_1 = parent.join(format!("{}.1", stem));
    let _ = fs::rename(log_path, &archive_1);
    Ok(())
}

pub fn get_log_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("logs").join("serchx.log")
}

pub fn init_logging(
    app_data_dir: &Path,
    log_buffer: LogRingBuffer,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let logs_dir = app_data_dir.join("logs");
    let _ = fs::create_dir_all(&logs_dir);
    let log_file_path = logs_dir.join("serchx.log");

    if let Ok(metadata) = fs::metadata(&log_file_path) {
        if metadata.len() >= MAX_LOG_SIZE_BYTES {
            let _ = rotate_log_files(&log_file_path, MAX_LOG_ARCHIVES);
        }
    }

    let file_appender = tracing_appender::rolling::never(&logs_dir, "serchx.log");
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("serchx=debug,info,warn,error"));

    let stdout_layer = tracing_subscriber::fmt::layer().with_ansi(true);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false);

    let ring_buffer_writer = RingBufferWriter { buffer: log_buffer };
    let ring_layer = tracing_subscriber::fmt::layer()
        .with_writer(ring_buffer_writer)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .with(ring_layer);

    let _ = subscriber.try_init();

    tracing::info!(
        "Tracing logger initialized. Log file: {}",
        log_file_path.display()
    );

    Some(guard)
}
