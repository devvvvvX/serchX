use crate::ingest::IngestionError;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: WatchEventKind,
}

pub struct FileSystemWatcher {
    watcher: Option<RecommendedWatcher>,
    event_sender: Sender<WatchEvent>,
    event_receiver: Option<Receiver<WatchEvent>>,
    recent_events: Arc<Mutex<HashMap<PathBuf, Instant>>>,
    debounce_duration: Duration,
    watched_paths: HashSet<PathBuf>,
}

pub type FileWatcher = FileSystemWatcher;

impl Default for FileSystemWatcher {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

impl FileSystemWatcher {
    pub fn new(debounce_duration: Duration) -> Self {
        let (tx, rx) = channel();
        Self {
            watcher: None,
            event_sender: tx,
            event_receiver: Some(rx),
            recent_events: Arc::new(Mutex::new(HashMap::new())),
            debounce_duration,
            watched_paths: HashSet::new(),
        }
    }

    pub fn take_receiver(&mut self) -> Option<Receiver<WatchEvent>> {
        self.event_receiver.take()
    }

    pub fn is_ignored_path(path: &Path) -> bool {
        crate::safety::FileFilter::default()
            .is_ignored_path(path)
            .is_some()
    }

    fn get_or_create_watcher(&mut self) -> Result<&mut RecommendedWatcher, IngestionError> {
        if self.watcher.is_none() {
            let event_sender = self.event_sender.clone();
            let recent_events = self.recent_events.clone();
            let debounce = self.debounce_duration;

            let watcher = RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| match res {
                    Ok(event) => {
                        let kind = match event.kind {
                            EventKind::Create(_) => Some(WatchEventKind::Created),
                            EventKind::Modify(_) => Some(WatchEventKind::Modified),
                            EventKind::Remove(_) => Some(WatchEventKind::Deleted),
                            _ => None,
                        };

                        if let Some(event_kind) = kind {
                            for path in event.paths {
                                if Self::is_ignored_path(&path) {
                                    continue;
                                }

                                // Debounce logic
                                let now = Instant::now();
                                let mut recent = recent_events.lock().unwrap();
                                if let Some(last_time) = recent.get(&path) {
                                    if now.duration_since(*last_time) < debounce {
                                        continue;
                                    }
                                }
                                recent.insert(path.clone(), now);

                                tracing::debug!("Watcher raw event sent for {:?}", path);
                                let _ = event_sender.send(WatchEvent {
                                    path,
                                    kind: event_kind.clone(),
                                });
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!("Watcher error: {:?}", err);
                    }
                },
                Config::default(),
            )
            .map_err(|e| IngestionError::Watcher(e.to_string()))?;

            self.watcher = Some(watcher);
        }
        Ok(self.watcher.as_mut().unwrap())
    }

    pub fn watch_directory<P: AsRef<Path>>(&mut self, dir_path: P) -> Result<(), IngestionError> {
        let dir = dir_path.as_ref().to_path_buf();
        if !dir.exists() {
            return Err(IngestionError::Watcher(format!(
                "Directory does not exist: {:?}",
                dir
            )));
        }

        if self.watched_paths.contains(&dir) {
            return Ok(());
        }

        let watcher = self.get_or_create_watcher()?;
        watcher
            .watch(&dir, RecursiveMode::Recursive)
            .map_err(|e| IngestionError::Watcher(e.to_string()))?;

        self.watched_paths.insert(dir);
        Ok(())
    }

    pub fn unwatch_directory<P: AsRef<Path>>(&mut self, dir_path: P) -> Result<(), IngestionError> {
        let dir = dir_path.as_ref().to_path_buf();
        if let Some(ref mut watcher) = self.watcher {
            let _ = watcher.unwatch(&dir);
        }
        self.watched_paths.remove(&dir);
        Ok(())
    }

    pub fn get_watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignored_path_filters() {
        assert!(FileSystemWatcher::is_ignored_path(Path::new(".DS_Store")));
        assert!(FileSystemWatcher::is_ignored_path(Path::new("/Users/foo/.git/config")));
        assert!(FileSystemWatcher::is_ignored_path(Path::new("/Users/foo/node_modules/pkg")));
        assert!(FileSystemWatcher::is_ignored_path(Path::new("/Users/foo/file.tmp")));
        assert!(!FileSystemWatcher::is_ignored_path(Path::new("/Users/foo/Documents/notes.txt")));
        assert!(!FileSystemWatcher::is_ignored_path(Path::new("/Users/foo/Screenshots/app.png")));
    }
}
