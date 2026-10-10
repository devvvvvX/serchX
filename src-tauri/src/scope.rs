use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultFolderScope {
    pub path: String,
    pub display_name: String,
    pub enabled_by_default: bool,
    pub is_opt_in: bool,
}

pub fn get_default_folder_scope() -> Vec<DefaultFolderScope> {
    let mut scopes = Vec::new();

    // 1. Documents (SCOPE-01: default enabled)
    if let Some(doc_dir) = dirs::document_dir() {
        scopes.push(DefaultFolderScope {
            path: doc_dir.to_string_lossy().to_string(),
            display_name: "Documents".to_string(),
            enabled_by_default: true,
            is_opt_in: false,
        });
    }

    // 2. Desktop (SCOPE-01: default enabled)
    if let Some(desk_dir) = dirs::desktop_dir() {
        scopes.push(DefaultFolderScope {
            path: desk_dir.to_string_lossy().to_string(),
            display_name: "Desktop".to_string(),
            enabled_by_default: true,
            is_opt_in: false,
        });
    }

    // 3. Downloads (SCOPE-02: explicit opt-in, disabled by default)
    if let Some(dl_dir) = dirs::download_dir() {
        scopes.push(DefaultFolderScope {
            path: dl_dir.to_string_lossy().to_string(),
            display_name: "Downloads".to_string(),
            enabled_by_default: false,
            is_opt_in: true,
        });
    }

    scopes
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_default_folders() {
        let scopes = get_default_folder_scope();
        assert!(!scopes.is_empty());

        let doc_scope = scopes.iter().find(|s| s.display_name == "Documents");
        assert!(doc_scope.is_some());
        let doc = doc_scope.unwrap();
        assert!(doc.enabled_by_default);
        assert!(!doc.is_opt_in);

        let desk_scope = scopes.iter().find(|s| s.display_name == "Desktop");
        assert!(desk_scope.is_some());
        let desk = desk_scope.unwrap();
        assert!(desk.enabled_by_default);
        assert!(!desk.is_opt_in);

        let dl_scope = scopes.iter().find(|s| s.display_name == "Downloads");
        assert!(dl_scope.is_some());
        let dl = dl_scope.unwrap();
        assert!(!dl.enabled_by_default, "Downloads must be disabled by default (SCOPE-02)");
        assert!(dl.is_opt_in, "Downloads must be opt-in (SCOPE-02)");
    }
}
