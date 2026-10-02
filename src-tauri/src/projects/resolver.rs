use super::{
    git, manifests,
    models::{Confidence, ProjectIdentity},
};
use crate::ports::models::PortEntry;
use std::path::{Path, PathBuf};

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
#[cfg(unix)]
fn device(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    path.metadata().ok().map(|m| m.dev())
}
#[cfg(not(unix))]
fn device(_: &Path) -> Option<u64> {
    None
}
// Ignore dependency/vendor manifests: their application is above node_modules/target.
pub fn find_root(start: &Path) -> Option<(PathBuf, Vec<String>)> {
    let mut path = crate::process::privacy::accessible_path(start).ok()?;
    if path.is_file() {
        path.pop();
    }
    let parts: Vec<_> = path.ancestors().collect();
    if let Some(deps) = parts.iter().find(|p| {
        p.file_name()
            .is_some_and(|n| n == "node_modules" || n == "target" || n == ".venv" || n == "vendor")
    }) {
        path = deps.parent()?.to_path_buf();
    }
    let home = home().and_then(|p| p.canonicalize().ok());
    let dev = device(&path);
    for candidate in path.ancestors().take(24) {
        if candidate.parent().is_none()
            || home.as_deref() == Some(candidate)
            || crate::process::privacy::blocked(candidate)
            || device(candidate) != dev
        {
            break;
        }
        let markers = manifests::markers(candidate);
        if !markers.is_empty()
            || crate::process::privacy::accessible_path(&candidate.join(".git")).is_ok()
        {
            return Some((candidate.to_path_buf(), markers));
        }
    }
    None
}
pub fn display_path(root: &Path) -> String {
    if let Some(home) = home() {
        if let Ok(rest) = root.strip_prefix(home) {
            return format!("~/{}", rest.to_string_lossy());
        }
    }
    root.to_string_lossy().to_string()
}
pub fn identity(root: &Path, markers: &[String]) -> ProjectIdentity {
    let manifest = manifests::resolve(root, markers);
    let git = git::resolve(root);
    let directory = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let raw = manifest
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(directory);
    ProjectIdentity {
        name: manifests::display_name(&raw),
        root_path: root.to_string_lossy().to_string(),
        display_path: display_path(root),
        project_type: manifest.ecosystem,
        frameworks: manifest.frameworks,
        repository: git.repository,
        git_branch: git.branch,
        git_dirty: git.dirty,
        confidence: Confidence::High,
        evidence: if git.root.is_some() {
            "Working directory inside a project / Git repository"
        } else {
            "Working directory inside a project manifest"
        }
        .into(),
        manifests: markers.to_vec(),
    }
}
pub fn candidate(
    entry: &PortEntry,
    parent_cwd: Option<&Path>,
) -> Option<(PathBuf, Vec<String>, Confidence, String)> {
    if entry.system {
        return None;
    }
    crate::process::privacy::trace(entry.pid.unwrap_or(0), "project_detection", "begin");
    if let Some(cwd) = &entry.cwd {
        if let Some((root, markers)) = find_root(Path::new(cwd)) {
            return Some((
                root,
                markers,
                Confidence::High,
                "Process working directory".into(),
            ));
        }
    }
    // Missing/unusable cwd is not permission to probe executables, argv paths,
    // parent application directories, or bundle contents.
    let _ = parent_cwd;
    None
}
pub fn service(entry: &PortEntry, project: Option<&ProjectIdentity>) -> String {
    let cmd = format!("{} {}", entry.process, entry.command.join(" ")).to_lowercase();
    for (needle, label) in [
        ("postgres", "PostgreSQL"),
        ("redis-server", "Redis"),
        ("mongod", "MongoDB"),
        ("docker", "Docker"),
        ("vite", "Vite"),
        ("next", "Next.js"),
        ("nuxt", "Nuxt"),
        ("django", "Django"),
        ("flask", "Flask"),
        ("uvicorn", "FastAPI / ASGI"),
        ("spring", "Spring"),
    ] {
        if cmd.contains(needle) {
            return label.into();
        }
    }
    if let Some(p) = project {
        if let Some(kind) = &p.project_type {
            return kind.clone();
        }
    }
    entry.process.clone()
}
