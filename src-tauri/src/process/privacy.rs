//! Conservative filesystem policy, not a TCC permission preflight.
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
// An errno cannot establish application-wide TCC authorization. Remember the
// failed resource across PIDs without claiming all filesystem access is denied.
fn denied() -> &'static Mutex<HashSet<PathBuf>> {
    static DENIED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    DENIED.get_or_init(Default::default)
}
pub fn explicit_retry() {
    denied().lock().unwrap().clear();
}
pub fn observe_error(path: &Path, error: &std::io::Error) {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        denied().lock().unwrap().insert(path.to_path_buf());
        trace(0, "project_filesystem", "permission_denied");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessState {
    Available,
    PermissionDenied,
    ProtectedResource,
    Unsupported,
    Unknown,
}
#[derive(Debug)]
pub enum InspectionError {
    ProcessExited,
    PermissionDenied,
    ProtectedResource,
    Unsupported,
    Io(std::io::Error),
}
impl From<std::io::Error> for InspectionError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::NotFound => Self::ProcessExited,
            std::io::ErrorKind::Unsupported => Self::Unsupported,
            _ => Self::Io(e),
        }
    }
}
impl InspectionError {
    pub fn state(&self) -> AccessState {
        match self {
            Self::PermissionDenied => AccessState::PermissionDenied,
            Self::ProtectedResource => AccessState::ProtectedResource,
            Self::Unsupported => AccessState::Unsupported,
            _ => AccessState::Unknown,
        }
    }
}

// No paths, argv, or environment values are written to diagnostics.
pub fn trace(pid: u32, operation: &str, result: &str) {
    #[cfg(debug_assertions)]
    if std::env::var_os("PORT_AUTHORITY_TRACE_INSPECTION").is_some() {
        eprintln!("[process-inspection] pid={pid} operation={operation} result={result}");
    }
    let _ = (pid, operation, result);
}

pub fn blocked(path: &Path) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    !path.is_absolute()
        || path.components().any(|c| match c {
            Component::ParentDir => true,
            Component::Normal(p) => {
                let s = p.to_string_lossy().to_ascii_lowercase();
                matches!(
                    s.as_str(),
                    "library"
                        | "applications"
                        | "system"
                        | "volumes"
                        | "documents"
                        | "desktop"
                        | "downloads"
                        | "pictures"
                        | "movies"
                        | "music"
                ) || s.ends_with(".app")
            }
            _ => false,
        })
}

/// Reject protected spellings before *any* filesystem query. Walk symlinks one
/// component at a time so canonicalize cannot traverse into a protected target.
pub fn accessible_path(path: &Path) -> Result<PathBuf, InspectionError> {
    if denied().lock().unwrap().iter().any(|p| path.starts_with(p)) {
        return Err(InspectionError::PermissionDenied);
    }
    if blocked(path) {
        trace(0, "project_path", "protected_resource");
        return Err(InspectionError::ProtectedResource);
    }
    #[cfg(target_os = "macos")]
    {
        // macOS's standard /tmp and /var aliases are known spellings, not
        // untrusted project symlinks. Normalize them without following links.
        let normalized;
        let path = if let Ok(rest) = path.strip_prefix("/var") {
            normalized = Path::new("/private/var").join(rest);
            &normalized
        } else if let Ok(rest) = path.strip_prefix("/tmp") {
            normalized = Path::new("/private/tmp").join(rest);
            &normalized
        } else {
            path
        };
        let mut current = PathBuf::new();
        for c in path.components() {
            current.push(c);
            let meta =
                std::fs::symlink_metadata(&current).inspect_err(|e| observe_error(&current, e))?;
            if meta.file_type().is_symlink() {
                // Don't follow aliases during automatic inference, even if their
                // destination looks safe. User launch paths can be canonical already.
                return Err(InspectionError::ProtectedResource);
            }
        }
        Ok(current)
    }
    #[cfg(not(target_os = "macos"))]
    Ok(path.canonicalize()?)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn protected_spellings_are_rejected_before_filesystem_lookup() {
        for path in [
            "/Users/test/Library/Containers/app/data",
            "/Users/test/Library/Group Containers/app",
            "/Users/test/Library/Application Support/app",
            "/Applications/Example.app/Contents",
            "/anywhere/Example.app/Contents",
            "/Users/test/Code/../Library/app",
            "/Users/test/Documents/project",
        ] {
            assert!(matches!(
                accessible_path(Path::new(path)),
                Err(InspectionError::ProtectedResource)
            ));
        }
    }
    #[test]
    fn a_project_alias_cannot_redirect_discovery_into_app_data() {
        let root = std::env::temp_dir().join(format!("pa-privacy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink("/Applications/Example.app/Contents", root.join("alias"))
            .unwrap();
        assert!(matches!(
            accessible_path(&root.join("alias/child")),
            Err(InspectionError::ProtectedResource)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
