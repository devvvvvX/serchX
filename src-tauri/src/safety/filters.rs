use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

pub const DEFAULT_MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
pub const SF_DATALESS: u32 = 0x4000_0000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterRejectionReason {
    #[error("Directory '{0}' is blacklisted")]
    IgnoredDirectory(String),
    #[error("File extension '{0}' is not in allowlist")]
    DisallowedExtension(String),
    #[error("File extension '{0}' is blacklisted")]
    BlacklistedExtension(String),
    #[error("File size {size} exceeds cap {max} bytes")]
    FileTooLarge { size: u64, max: u64 },
    #[error("File is an evicted iCloud placeholder (dataless)")]
    DatalessICloudPlaceholder,
    #[error("Hidden file '{0}'")]
    HiddenFile(String),
    #[error("Temporary file '{0}'")]
    TemporaryFile(String),
}

#[derive(Debug, Clone)]
pub struct FileFilter {
    pub allowed_extensions: HashSet<String>,
    pub blacklisted_dir_names: HashSet<String>,
    pub blacklisted_extensions: HashSet<String>,
    pub max_file_size_bytes: u64,
    /// Absolute paths that should never be indexed (e.g. app's own data directory).
    pub excluded_paths: Vec<PathBuf>,
}

impl Default for FileFilter {
    fn default() -> Self {
        let allowed = [
            // Documents
            "pdf", "docx", "txt", "md", "markdown", "rst", "rtf", "log", "csv", "tsv",
            // Config & Data
            "json", "yaml", "yml", "toml", "xml", "html", "htm", "sql", // Code
            "rs", "py", "js", "ts", "tsx", "jsx", "c", "cpp", "cc", "h", "hpp", "go", "java", "kt",
            "swift", "rb", "php", "sh", "zsh", "bash", "css", "scss",
            // Images (Apple Vision OCR)
            "png", "jpg", "jpeg", "webp", "tiff", "tif", "bmp", "heic",
        ];

        let blacklisted_dirs = [
            ".git",
            "node_modules",
            "target",
            "build",
            "dist",
            "out",
            "Caches",
            "Pods",
            ".trash",
            ".Trash",
            ".venv",
            "venv",
            "__pycache__",
            ".idea",
            ".vscode",
            ".next",
            ".nuxt",
            ".gradle",
            "Library/Caches",
        ];

        let blacklisted_exts = [
            "app", "dmg", "pkg", "iso", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "mp4", "mov",
            "avi", "mkv", "wmv", "mp3", "wav", "flac", "m4a", "aac", "bin", "exe", "dylib", "so",
            "a", "o", "dll", "class", "pyc",
        ];

        Self {
            allowed_extensions: allowed.into_iter().map(|s| s.to_lowercase()).collect(),
            blacklisted_dir_names: blacklisted_dirs
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            blacklisted_extensions: blacklisted_exts
                .into_iter()
                .map(|s| s.to_lowercase())
                .collect(),
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE,
            excluded_paths: Vec::new(),
        }
    }
}

impl FileFilter {
    /// Creates a filter with specific absolute paths excluded from indexing.
    /// Use this to prevent the app from indexing its own data directory.
    pub fn with_excluded_paths(excluded_paths: Vec<PathBuf>) -> Self {
        let mut filter = Self::default();
        filter.excluded_paths = excluded_paths;
        filter
    }

    pub fn is_ignored_directory_name(&self, name: &str) -> bool {
        self.blacklisted_dir_names.contains(name)
    }

    pub fn is_ignored_path(&self, path: &Path) -> Option<FilterRejectionReason> {
        // Check if path falls inside any excluded absolute path (e.g. app's own data dir)
        for excluded in &self.excluded_paths {
            if is_path_inside_or_equal(path, excluded) {
                return Some(FilterRejectionReason::IgnoredDirectory(
                    excluded.to_string_lossy().to_string(),
                ));
            }
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.') && file_name != "." && file_name != ".." {
                return Some(FilterRejectionReason::HiddenFile(file_name.to_string()));
            }
            if file_name.ends_with('~')
                || file_name.ends_with(".tmp")
                || file_name.starts_with("~$")
            {
                return Some(FilterRejectionReason::TemporaryFile(file_name.to_string()));
            }
        }

        for component in path.components() {
            if let std::path::Component::Normal(comp) = component {
                if let Some(s) = comp.to_str() {
                    if self.is_ignored_directory_name(s) {
                        return Some(FilterRejectionReason::IgnoredDirectory(s.to_string()));
                    }
                    if s.ends_with(".app") {
                        return Some(FilterRejectionReason::IgnoredDirectory(s.to_string()));
                    }
                }
            }
        }
        None
    }

    pub fn is_dataless(metadata: &Metadata) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt;
            (metadata.st_flags() & SF_DATALESS) != 0
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = metadata;
            false
        }
    }

    pub fn should_index_path(
        &self,
        path: &Path,
        metadata: &Metadata,
    ) -> Result<(), FilterRejectionReason> {
        // 1. Directory and file name checks
        if let Some(reason) = self.is_ignored_path(path) {
            return Err(reason);
        }

        // 2. Pre-read size cap check
        if metadata.len() > self.max_file_size_bytes {
            return Err(FilterRejectionReason::FileTooLarge {
                size: metadata.len(),
                max: self.max_file_size_bytes,
            });
        }

        // 3. macOS iCloud evicted dataless placeholder check
        if Self::is_dataless(metadata) {
            return Err(FilterRejectionReason::DatalessICloudPlaceholder);
        }

        // 4. Extension checks
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext.is_empty() {
            return Err(FilterRejectionReason::DisallowedExtension(
                "<no extension>".to_string(),
            ));
        }

        if self.blacklisted_extensions.contains(&ext) {
            return Err(FilterRejectionReason::BlacklistedExtension(ext));
        }

        if !self.allowed_extensions.contains(&ext) {
            return Err(FilterRejectionReason::DisallowedExtension(ext));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathPermissionResult {
    Allowed,
    BlockedByBlacklist { blacklisted_root: PathBuf },
    OutsideWatchedRoots,
}

/// Helper that checks if `child` is equal to or inside `ancestor`.
pub fn is_path_inside_or_equal(child: &Path, ancestor: &Path) -> bool {
    if child == ancestor {
        return true;
    }
    child.starts_with(ancestor)
}

/// Returns true if `folder_path` resides inside any active blacklisted root.
pub fn is_watched_folder_blacklist_override(
    folder_path: &Path,
    blacklisted_roots: &[PathBuf],
) -> bool {
    blacklisted_roots
        .iter()
        .any(|b| folder_path != b.as_path() && is_path_inside_or_equal(folder_path, b))
}

/// Resolves whether `candidate_path` is permitted to be crawled and indexed based on longest-prefix matching.
///
/// If candidate_path matches watched root W_max and blacklisted root B_max:
/// If W_max has strictly more path components than B_max, candidate_path is inside an explicit child watch
/// that overrides the ancestor blacklist -> Allowed.
/// Otherwise, the blacklist applies -> BlockedByBlacklist.
pub fn resolve_path_indexing_permission(
    candidate_path: &Path,
    watched_roots: &[PathBuf],
    blacklisted_roots: &[PathBuf],
) -> PathPermissionResult {
    let matching_watched: Vec<&PathBuf> = watched_roots
        .iter()
        .filter(|w| is_path_inside_or_equal(candidate_path, w))
        .collect();

    if matching_watched.is_empty() {
        return PathPermissionResult::OutsideWatchedRoots;
    }

    let matching_blacklisted: Vec<&PathBuf> = blacklisted_roots
        .iter()
        .filter(|b| is_path_inside_or_equal(candidate_path, b))
        .collect();

    if matching_blacklisted.is_empty() {
        return PathPermissionResult::Allowed;
    }

    let w_max = matching_watched
        .into_iter()
        .max_by_key(|w| w.components().count())
        .unwrap();

    let b_max = matching_blacklisted
        .into_iter()
        .max_by_key(|b| b.components().count())
        .unwrap();

    if w_max.components().count() > b_max.components().count() {
        // Child watched folder explicitly overrides ancestor blacklist!
        PathPermissionResult::Allowed
    } else {
        PathPermissionResult::BlockedByBlacklist {
            blacklisted_root: (*b_max).clone(),
        }
    }
}

/// Determines whether a directory entry should be skipped during recursive traversal.
/// Returns true if directory should be skipped (pruned).
pub fn should_skip_directory(
    dir_path: &Path,
    watched_roots: &[PathBuf],
    blacklisted_roots: &[PathBuf],
) -> bool {
    let matching_blacklisted: Vec<&PathBuf> = blacklisted_roots
        .iter()
        .filter(|b| is_path_inside_or_equal(dir_path, b))
        .collect();

    if matching_blacklisted.is_empty() {
        return false;
    }

    // It is blacklisted. But is there any watched root that is inside dir_path (a nested exception)
    // OR is dir_path inside a watched root that overrides this blacklist?
    let max_blacklist_depth = matching_blacklisted
        .iter()
        .map(|b| b.components().count())
        .max()
        .unwrap_or(0);

    let has_watched_inside_or_override = watched_roots.iter().any(|w| {
        // Either watched root is inside dir_path (need to descend to reach it)
        is_path_inside_or_equal(w, dir_path)
            // Or dir_path is inside an overriding child watched root
            || (is_path_inside_or_equal(dir_path, w) && w.components().count() > max_blacklist_depth)
    });

    !has_watched_inside_or_override
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_size_and_extension_filters() {
        let filter = FileFilter::default();

        let allowed_path = Path::new("/Users/test/Documents/notes.md");
        assert!(filter.is_ignored_path(allowed_path).is_none());

        let temp_dir = tempfile::tempdir().unwrap();
        let small_file = temp_dir.path().join("report.docx");
        {
            let mut f = std::fs::File::create(&small_file).unwrap();
            f.write_all(b"sample text").unwrap();
        }
        let meta = std::fs::metadata(&small_file).unwrap();
        assert!(filter.should_index_path(&small_file, &meta).is_ok());

        // File too large
        let mut custom_filter = FileFilter::default();
        custom_filter.max_file_size_bytes = 10;
        let large_file = temp_dir.path().join("large.txt");
        {
            let mut f = std::fs::File::create(&large_file).unwrap();
            f.write_all(b"this is more than 10 bytes of text").unwrap();
        }
        let large_meta = std::fs::metadata(&large_file).unwrap();
        match custom_filter.should_index_path(&large_file, &large_meta) {
            Err(FilterRejectionReason::FileTooLarge { size, max }) => {
                assert!(size > 10);
                assert_eq!(max, 10);
            }
            res => panic!("Expected FileTooLarge, got {:?}", res),
        }

        // Blacklisted extension
        let dmg_file = temp_dir.path().join("installer.dmg");
        {
            let mut f = std::fs::File::create(&dmg_file).unwrap();
            f.write_all(b"dmg bytes").unwrap();
        }
        let dmg_meta = std::fs::metadata(&dmg_file).unwrap();
        assert!(matches!(
            filter.should_index_path(&dmg_file, &dmg_meta),
            Err(FilterRejectionReason::BlacklistedExtension(_))
        ));

        // Disallowed unknown extension
        let unknown_file = temp_dir.path().join("data.xyz123");
        {
            let mut f = std::fs::File::create(&unknown_file).unwrap();
            f.write_all(b"xyz bytes").unwrap();
        }
        let unknown_meta = std::fs::metadata(&unknown_file).unwrap();
        assert!(matches!(
            filter.should_index_path(&unknown_file, &unknown_meta),
            Err(FilterRejectionReason::DisallowedExtension(_))
        ));
    }

    #[test]
    fn test_blacklisted_and_dataless_skips() {
        let filter = FileFilter::default();

        let git_path = Path::new("/Users/test/repo/.git/HEAD");
        assert!(matches!(
            filter.is_ignored_path(git_path),
            Some(FilterRejectionReason::IgnoredDirectory(d)) if d == ".git"
        ));

        let node_modules_path = Path::new("/Users/test/app/node_modules/lodash/index.js");
        assert!(matches!(
            filter.is_ignored_path(node_modules_path),
            Some(FilterRejectionReason::IgnoredDirectory(d)) if d == "node_modules"
        ));

        let target_path = Path::new("/Users/test/rust_app/target/release/build.rs");
        assert!(matches!(
            filter.is_ignored_path(target_path),
            Some(FilterRejectionReason::IgnoredDirectory(d)) if d == "target"
        ));

        let app_bundle_path = Path::new("/Applications/Slack.app/Contents/Info.plist");
        assert!(matches!(
            filter.is_ignored_path(app_bundle_path),
            Some(FilterRejectionReason::IgnoredDirectory(_))
        ));

        let hidden_file = Path::new("/Users/test/.DS_Store");
        assert!(matches!(
            filter.is_ignored_path(hidden_file),
            Some(FilterRejectionReason::HiddenFile(_))
        ));

        let temp_file = Path::new("/Users/test/document.docx.tmp");
        assert!(matches!(
            filter.is_ignored_path(temp_file),
            Some(FilterRejectionReason::TemporaryFile(_))
        ));
    }
}
