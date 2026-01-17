use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("Path must be relative: {0}")]
    AbsolutePath(String),

    #[error("Path traversal not allowed: {0}")]
    PathTraversal(String),

    #[error("Path must be under workspace/: {0}")]
    OutsideWorkspace(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// 解析並驗證工作區路徑
/// 只允許 workspace/ 之下的相對路徑
pub fn resolve_workspace_path(base_dir: &Path, relative_path: &str) -> Result<PathBuf, PathError> {
    // 檢查是否為絕對路徑
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(PathError::AbsolutePath(relative_path.to_string()));
    }

    // 檢查路徑遍歷
    if relative_path.contains("..") {
        return Err(PathError::PathTraversal(relative_path.to_string()));
    }

    // 檢查是否以 workspace/ 開頭
    if !relative_path.starts_with("workspace/") && !relative_path.starts_with("workspace\\") {
        return Err(PathError::OutsideWorkspace(relative_path.to_string()));
    }

    // 組合完整路徑
    let full_path = base_dir.join(relative_path);
    Ok(full_path)
}

/// 驗證檔案存在
pub fn ensure_file_exists(path: &Path) -> Result<(), PathError> {
    if !path.exists() {
        return Err(PathError::NotFound(path.display().to_string()));
    }
    Ok(())
}

/// 確保目錄存在，若不存在則建立
pub fn ensure_dir_exists(path: &Path) -> Result<(), PathError> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_relative_path() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/inputs/slide.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_absolute_path() {
        let base = PathBuf::from("/project");
        // Use Windows-style absolute path for cross-platform compatibility
        #[cfg(windows)]
        let result = resolve_workspace_path(&base, "C:\\Windows\\System32\\config");
        #[cfg(not(windows))]
        let result = resolve_workspace_path(&base, "/etc/passwd");
        assert!(matches!(result, Err(PathError::AbsolutePath(_))));
    }

    #[test]
    fn test_reject_path_traversal() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/../../../etc/passwd");
        assert!(matches!(result, Err(PathError::PathTraversal(_))));
    }

    #[test]
    fn test_reject_outside_workspace() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "src/main.rs");
        assert!(matches!(result, Err(PathError::OutsideWorkspace(_))));
    }

    #[test]
    fn test_valid_nested_path() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/themes/default.json");
        assert!(result.is_ok());
    }
}
