use super::catalog::ModelCatalogItem;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub speed_mbps: f32,
    pub eta_seconds: u64,
    pub status: String,
    pub error_message: Option<String>,
}

/// Tracks per-model download state so pause/resume works correctly.
///
/// Design: Pause does NOT hold the HTTP connection open (servers drop idle
/// connections after 30-120s). Instead, pause sets a flag that causes the
/// download loop to exit cleanly, flushing the .part file. Resume starts a
/// fresh HTTP request with a Range header from where the .part file left off.
#[derive(Clone, Default)]
pub struct DownloadManager {
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// When true, the download loop should stop gracefully (used by pause).
    /// Both pause and cancel set this; cancel also fires the CancellationToken.
    stop_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// Tracks whether a model is paused (so the frontend can query state
    /// and resume_model_download knows to restart the download).
    paused_models: Arc<Mutex<HashMap<String, bool>>>,
    /// In-memory cache of the latest DownloadProgress per model, allowing
    /// the frontend to poll progress via IPC without relying solely on window events.
    active_progress: Arc<Mutex<HashMap<String, DownloadProgress>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            stop_flags: Arc::new(Mutex::new(HashMap::new())),
            paused_models: Arc::new(Mutex::new(HashMap::new())),
            active_progress: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn update_progress(&self, progress: &DownloadProgress) {
        let mut map = self.active_progress.lock().unwrap();
        map.insert(progress.model_id.clone(), progress.clone());
    }

    pub fn get_progress(&self, model_id: &str) -> Option<DownloadProgress> {
        let map = self.active_progress.lock().unwrap();
        map.get(model_id).cloned()
    }

    pub fn register_cancel_token(&self, model_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        let mut map = self.cancellations.lock().unwrap();
        map.insert(model_id.to_string(), token.clone());
        token
    }

    /// Register a stop flag for the download loop. Returns an Arc<AtomicBool>
    /// that the download loop checks on each chunk.
    fn register_stop_flag(&self, model_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut map = self.stop_flags.lock().unwrap();
        map.insert(model_id.to_string(), flag.clone());
        flag
    }

    /// Cancel a download permanently. The .part file is preserved on disk
    /// so a future start_model_download can resume from the same offset.
    pub fn cancel(&self, model_id: &str) -> bool {
        // Clear paused state
        {
            let mut paused = self.paused_models.lock().unwrap();
            paused.remove(model_id);
        }
        // Set stop flag so the download loop exits
        {
            let map = self.stop_flags.lock().unwrap();
            if let Some(flag) = map.get(model_id) {
                flag.store(true, Ordering::SeqCst);
            }
        }
        // Fire the cancellation token
        let mut map = self.cancellations.lock().unwrap();
        if let Some(token) = map.remove(model_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Pause a download. This sets the stop flag so the download loop exits
    /// gracefully after flushing the .part file. The HTTP connection is closed.
    /// Resume will start a new connection with Range headers.
    pub fn pause(&self, model_id: &str) -> bool {
        // Set stop flag so download loop exits
        let stopped = {
            let map = self.stop_flags.lock().unwrap();
            if let Some(flag) = map.get(model_id) {
                flag.store(true, Ordering::SeqCst);
                true
            } else {
                false
            }
        };
        if stopped {
            let mut paused = self.paused_models.lock().unwrap();
            paused.insert(model_id.to_string(), true);

            // Immediately update active_progress to "paused" so the frontend
            // poll gets the correct state before the download loop exits.
            let mut progress_map = self.active_progress.lock().unwrap();
            if let Some(p) = progress_map.get_mut(model_id) {
                p.status = "paused".to_string();
                p.speed_mbps = 0.0;
                p.eta_seconds = 0;
            }
        }
        stopped
    }

    /// Check if a model download is currently paused.
    pub fn is_paused(&self, model_id: &str) -> bool {
        let paused = self.paused_models.lock().unwrap();
        paused.get(model_id).copied().unwrap_or(false)
    }

    /// Clear pause state (called when resuming or when download finishes).
    fn clear_pause(&self, model_id: &str) {
        let mut paused = self.paused_models.lock().unwrap();
        paused.remove(model_id);
    }

    pub fn remove_token(&self, model_id: &str) {
        let mut map = self.cancellations.lock().unwrap();
        map.remove(model_id);
        let mut stop_map = self.stop_flags.lock().unwrap();
        stop_map.remove(model_id);
    }

    pub async fn download_model<F>(
        &self,
        model: &ModelCatalogItem,
        target_dir: &Path,
        progress_callback: F,
    ) -> Result<PathBuf, String>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        self.clear_pause(&model.id);
        let cancel_token = self.register_cancel_token(&model.id);
        let stop_flag = self.register_stop_flag(&model.id);

        let dm_self = self.clone();
        let wrapped_callback = move |p: DownloadProgress| {
            dm_self.update_progress(&p);
            progress_callback(p);
        };

        let res = Self::download_file_internal(
            model,
            target_dir,
            wrapped_callback,
            cancel_token,
            stop_flag,
        )
        .await;
        self.remove_token(&model.id);
        res
    }

    async fn download_file_internal<F>(
        model: &ModelCatalogItem,
        target_dir: &Path,
        progress_callback: F,
        cancel_token: CancellationToken,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<PathBuf, String>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        std::fs::create_dir_all(target_dir)
            .map_err(|e| format!("Failed to create models dir: {}", e))?;
        let final_path = target_dir.join(&model.filename);
        let part_path = target_dir.join(format!("{}.part", model.filename));

        // If file already exists and has reasonable size, mark complete
        if final_path.exists() {
            if let Ok(meta) = std::fs::metadata(&final_path) {
                if meta.len() > 10_000_000 {
                    progress_callback(DownloadProgress {
                        model_id: model.id.clone(),
                        bytes_downloaded: meta.len(),
                        total_bytes: meta.len(),
                        percentage: 100.0,
                        speed_mbps: 0.0,
                        eta_seconds: 0,
                        status: "complete".to_string(),
                        error_message: None,
                    });
                    return Ok(final_path);
                }
            }
        }

        // Check for existing .part file to resume from
        let mut existing_bytes = 0u64;
        if part_path.exists() {
            if let Ok(meta) = std::fs::metadata(&part_path) {
                existing_bytes = meta.len();
            }
        }

        // Emit initial connecting progress so the UI gets instant feedback
        progress_callback(DownloadProgress {
            model_id: model.id.clone(),
            bytes_downloaded: existing_bytes,
            total_bytes: model.download_size_bytes,
            percentage: if model.download_size_bytes > 0 {
                ((existing_bytes as f32 / model.download_size_bytes as f32) * 100.0)
                    .clamp(0.0, 99.9)
            } else {
                0.0
            },
            speed_mbps: 0.0,
            eta_seconds: 0,
            status: "connecting".to_string(),
            error_message: None,
        });

        // Use connect_timeout and read_timeout — NOT a total request timeout.
        // A total timeout kills multi-GB downloads regardless of progress.
        // read_timeout guards against CDN stalls where connection succeeds
        // but data stops flowing.
        let client = reqwest::Client::builder()
            .user_agent("serchX/1.0 (Desktop App; macOS)")
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(
            "Starting download for model {} from URL: {} (existing_bytes: {})",
            model.id,
            model.download_url,
            existing_bytes
        );

        let mut req = client.get(&model.download_url);
        if existing_bytes > 0 {
            req = req.header("Range", format!("bytes={}-", existing_bytes));
            tracing::info!(
                "Resuming download for {} from byte {}",
                model.id,
                existing_bytes
            );
        }

        // Apply a 30s timeout specifically to the initial connection + header handshake,
        // so we never hang indefinitely waiting for the server response headers.
        let mut response = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            req.send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                let err = format!("Network request failed: {}", e);
                tracing::error!("HTTP request failed for {}: {}", model.id, err);
                return Err(err);
            }
            Err(_) => {
                let err = "Connection timed out waiting for server response (30s). Please check your internet connection.".to_string();
                tracing::error!("HTTP connection timeout for {}", model.id);
                return Err(err);
            }
        };

        // If server returns 416 (Range Not Satisfiable), existing .part file is stale/corrupt.
        // Reset .part file and retry fresh.
        if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing_bytes > 0 {
            tracing::warn!("Server returned 416 Range Not Satisfiable. Resetting .part file and retrying fresh.");
            let _ = std::fs::remove_file(&part_path);
            existing_bytes = 0;
            let fresh_req = client.get(&model.download_url);
            response =
                match tokio::time::timeout(std::time::Duration::from_secs(30), fresh_req.send())
                    .await
                {
                    Ok(Ok(resp)) => resp,
                    Ok(Err(e)) => return Err(format!("Network request failed: {}", e)),
                    Err(_) => {
                        return Err(
                            "Connection timed out waiting for server response (30s)".to_string()
                        )
                    }
                };
        }

        let status = response.status();
        tracing::info!(
            "HTTP response for {}: status={}, content_length={:?}",
            model.id,
            status,
            response.content_length()
        );

        let (mut file, mut downloaded_bytes, total_bytes) = if status
            == reqwest::StatusCode::PARTIAL_CONTENT
        {
            // Server supports Range — append to existing .part file
            let content_len = response.content_length().unwrap_or(0);
            let total = existing_bytes + content_len;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part_path)
                .map_err(|e| format!("Failed to open .part file for append: {}", e))?;
            (file, existing_bytes, total)
        } else if status.is_success() {
            // Server doesn't support Range or fresh download — start from scratch
            let total = response
                .content_length()
                .unwrap_or(model.download_size_bytes);
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&part_path)
                .map_err(|e| format!("Failed to open .part file: {}", e))?;
            (file, 0u64, total)
        } else {
            let err = match status {
                reqwest::StatusCode::NOT_FOUND => {
                    "Model file not found on Hugging Face (404 Not Found). The download URL may be invalid.".to_string()
                }
                reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::UNAUTHORIZED => {
                    format!("Access denied by Hugging Face ({}). The repository may be gated or private.", status)
                }
                _ => format!("Server returned HTTP status {}", status),
            };
            tracing::error!("{}", err);
            return Err(err);
        };

        // Emit initial progress so the UI shows the download bar immediately
        {
            let pct = if total_bytes > 0 {
                (downloaded_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                0.0
            };
            progress_callback(DownloadProgress {
                model_id: model.id.clone(),
                bytes_downloaded: downloaded_bytes,
                total_bytes,
                percentage: pct,
                speed_mbps: 0.0,
                eta_seconds: 0,
                status: "downloading".to_string(),
                error_message: None,
            });
        }

        let mut stream = response.bytes_stream();
        let mut last_progress_instant = Instant::now();
        let mut bytes_since_last_calc = 0u64;

        while let Some(chunk_result) = stream.next().await {
            // Check cancel first
            if cancel_token.is_cancelled() {
                let _ = file.flush();
                progress_callback(DownloadProgress {
                    model_id: model.id.clone(),
                    bytes_downloaded: downloaded_bytes,
                    total_bytes,
                    percentage: if total_bytes > 0 {
                        (downloaded_bytes as f32 / total_bytes as f32) * 100.0
                    } else {
                        0.0
                    },
                    speed_mbps: 0.0,
                    eta_seconds: 0,
                    status: "cancelled".to_string(),
                    error_message: Some("Download cancelled by user".to_string()),
                });
                return Err("Download cancelled".to_string());
            }

            // Check stop flag (set by pause). Flush .part file and return a
            // special "paused" error so the caller knows this is not a failure.
            if stop_flag.load(Ordering::SeqCst) && !cancel_token.is_cancelled() {
                let _ = file.flush();
                let pause_pct = if total_bytes > 0 {
                    (downloaded_bytes as f32 / total_bytes as f32) * 100.0
                } else {
                    0.0
                };
                progress_callback(DownloadProgress {
                    model_id: model.id.clone(),
                    bytes_downloaded: downloaded_bytes,
                    total_bytes,
                    percentage: pause_pct,
                    speed_mbps: 0.0,
                    eta_seconds: 0,
                    status: "paused".to_string(),
                    error_message: None,
                });
                // Return a sentinel error that the caller can distinguish
                return Err("Download paused".to_string());
            }

            let chunk = chunk_result.map_err(|e| format!("Error receiving stream chunk: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("Failed to write chunk to disk: {}", e))?;
            downloaded_bytes += chunk.len() as u64;
            bytes_since_last_calc += chunk.len() as u64;

            let elapsed = last_progress_instant.elapsed();
            if elapsed.as_millis() >= 150 {
                let seconds = elapsed.as_secs_f32();
                let speed_mbps =
                    ((bytes_since_last_calc as f32 * 8.0) / (1_000_000.0 * seconds)).max(0.0);
                bytes_since_last_calc = 0;
                last_progress_instant = Instant::now();

                let remaining_bytes = total_bytes.saturating_sub(downloaded_bytes);
                let eta_seconds = if speed_mbps > 0.05 {
                    ((remaining_bytes as f32 * 8.0) / (speed_mbps * 1_000_000.0)) as u64
                } else {
                    0
                };

                let percentage = if total_bytes > 0 {
                    ((downloaded_bytes as f32 / total_bytes as f32) * 100.0).clamp(0.0, 99.9)
                } else {
                    0.0
                };

                progress_callback(DownloadProgress {
                    model_id: model.id.clone(),
                    bytes_downloaded: downloaded_bytes,
                    total_bytes,
                    percentage,
                    speed_mbps,
                    eta_seconds,
                    status: "downloading".to_string(),
                    error_message: None,
                });
            }
        }

        file.flush()
            .map_err(|e| format!("Failed to flush file: {}", e))?;

        // Verification phase
        progress_callback(DownloadProgress {
            model_id: model.id.clone(),
            bytes_downloaded: downloaded_bytes,
            total_bytes,
            percentage: 99.9,
            speed_mbps: 0.0,
            eta_seconds: 0,
            status: "verifying".to_string(),
            error_message: None,
        });

        // Compute SHA-256 if expected hash is provided
        if !model.sha256.is_empty() {
            let mut check_file = std::fs::File::open(&part_path)
                .map_err(|e| format!("Failed to open .part file for hashing: {}", e))?;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let n = check_file
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error during hash: {}", e))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            let hex_hash = format!("{:x}", result);

            if !hex_hash.eq_ignore_ascii_case(&model.sha256) {
                let _ = std::fs::remove_file(&part_path);
                let err_msg = format!(
                    "Hash mismatch. Expected: {}, Got: {}",
                    model.sha256, hex_hash
                );
                progress_callback(DownloadProgress {
                    model_id: model.id.clone(),
                    bytes_downloaded: downloaded_bytes,
                    total_bytes,
                    percentage: 0.0,
                    speed_mbps: 0.0,
                    eta_seconds: 0,
                    status: "error".to_string(),
                    error_message: Some(err_msg.clone()),
                });
                return Err(err_msg);
            }
        }

        // Rename .part to .gguf
        std::fs::rename(&part_path, &final_path)
            .map_err(|e| format!("Failed to rename .part file: {}", e))?;

        progress_callback(DownloadProgress {
            model_id: model.id.clone(),
            bytes_downloaded: downloaded_bytes,
            total_bytes,
            percentage: 100.0,
            speed_mbps: 0.0,
            eta_seconds: 0,
            status: "complete".to_string(),
            error_message: None,
        });

        Ok(final_path)
    }

    pub fn delete_model(model_filename: &str, target_dir: &Path) -> Result<(), String> {
        let final_path = target_dir.join(model_filename);
        let part_path = target_dir.join(format!("{}.part", model_filename));

        let mut deleted = false;
        if final_path.exists() || final_path.symlink_metadata().is_ok() {
            std::fs::remove_file(&final_path)
                .map_err(|e| format!("Failed to delete {}: {}", model_filename, e))?;
            deleted = true;
        }
        if part_path.exists() || part_path.symlink_metadata().is_ok() {
            let _ = std::fs::remove_file(&part_path);
            deleted = true;
        }

        if deleted {
            Ok(())
        } else {
            Err("Model file not found on disk".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_delete_model() {
        let tmp = tempdir().unwrap();
        let model_file = tmp.path().join("test_model.gguf");
        std::fs::write(&model_file, b"sample model data").unwrap();

        assert!(model_file.exists());
        DownloadManager::delete_model("test_model.gguf", tmp.path()).unwrap();
        assert!(!model_file.exists());
    }

    #[test]
    fn test_cancellation_registration() {
        let manager = DownloadManager::new();
        let token = manager.register_cancel_token("test-model");
        assert!(!token.is_cancelled());
        assert!(manager.cancel("test-model"));
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_pause_state() {
        let manager = DownloadManager::new();
        // No active download — pause returns false
        assert!(!manager.pause("test-model"));
        assert!(!manager.is_paused("test-model"));

        // Register a stop flag (normally done inside download_model)
        let _flag = manager.register_stop_flag("test-model");
        assert!(manager.pause("test-model"));
        assert!(manager.is_paused("test-model"));

        manager.clear_pause("test-model");
        assert!(!manager.is_paused("test-model"));
    }
}
