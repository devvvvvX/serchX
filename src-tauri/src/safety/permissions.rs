use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum PermissionStatus {
    Granted,
    Denied { reason: String, fda_required: bool },
    NotFound,
}

pub fn check_folder_access(path: &Path) -> PermissionStatus {
    if !path.exists() {
        return PermissionStatus::NotFound;
    }

    match std::fs::read_dir(path) {
        Ok(_) => PermissionStatus::Granted,
        Err(e) => {
            if is_permission_error(&e) {
                PermissionStatus::Denied {
                    reason: format!(
                        "macOS Transparency, Consent, and Control (TCC) prevented access to {:?}. \
                        Grant Full Disk Access in System Settings > Privacy & Security > Full Disk Access.",
                        path
                    ),
                    fda_required: true,
                }
            } else if e.kind() == std::io::ErrorKind::NotFound {
                PermissionStatus::NotFound
            } else {
                PermissionStatus::Denied {
                    reason: format!("I/O access error: {}", e),
                    fda_required: false,
                }
            }
        }
    }
}

fn is_permission_error(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        return true;
    }
    if let Some(code) = e.raw_os_error() {
        // macOS: 1 = EPERM (Operation not permitted), 13 = EACCES (Permission denied)
        if code == 1 || code == 13 {
            return true;
        }
    }
    false
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_permission_checks() {
        // 1. Existing readable directory
        let temp_dir = tempfile::tempdir().unwrap();
        assert_eq!(
            check_folder_access(temp_dir.path()),
            PermissionStatus::Granted
        );

        // 2. Non-existent path
        let non_existent = temp_dir.path().join("does_not_exist_folder");
        assert_eq!(
            check_folder_access(&non_existent),
            PermissionStatus::NotFound
        );

        // 3. Permission error mapping
        let perm_err = std::io::Error::from_raw_os_error(1); // EPERM
        assert!(is_permission_error(&perm_err));

        let access_err = std::io::Error::from_raw_os_error(13); // EACCES
        assert!(is_permission_error(&access_err));

        let not_found_err = std::io::Error::from_raw_os_error(2); // ENOENT
        assert!(!is_permission_error(&not_found_err));
    }
}
