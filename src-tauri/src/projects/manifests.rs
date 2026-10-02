use std::{io::Read, path::Path};

pub const MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "Gemfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];
pub fn read_small(path: &Path) -> Option<String> {
    crate::process::privacy::accessible_path(path).ok()?;
    read_bounded(path, 256 * 1024)
}
pub fn read_bounded(path: &Path, limit: u64) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > limit {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    crate::process::privacy::trace(0, "read_manifest", "begin");
    let file = options
        .open(path)
        .inspect_err(|e| crate::process::privacy::observe_error(path, e))
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut value = String::new();
    file.take(limit + 1).read_to_string(&mut value).ok()?;
    (value.len() as u64 <= limit).then_some(value)
}

pub fn markers(path: &Path) -> Vec<String> {
    if crate::process::privacy::accessible_path(path).is_err() {
        return vec![];
    }
    let found: Vec<_> = MARKERS
        .iter()
        .filter(|m| {
            crate::process::privacy::accessible_path(&path.join(m)).is_ok_and(|p| p.is_file())
        })
        .map(|m| m.to_string())
        .collect();
    // Never enumerate arbitrary process directories. The previous read_dir here
    // blocked in opendir/open while macOS displayed the App Data consent dialog.
    let mut found = found;
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        for extension in ["sln", "csproj"] {
            let marker = format!("{name}.{extension}");
            if crate::process::privacy::accessible_path(&path.join(&marker))
                .is_ok_and(|p| p.is_file())
            {
                found.push(marker);
            }
        }
    }

    found
}
#[derive(Default)]
pub struct Manifest {
    pub name: Option<String>,
    pub ecosystem: Option<String>,
    pub frameworks: Vec<String>,
}
pub fn resolve(root: &Path, markers: &[String]) -> Manifest {
    let mut out = Manifest::default();
    for marker in markers {
        match marker.as_str() {
            "package.json" => {
                out.ecosystem = Some("Node.js".into());
                if let Some(v) = read_small(&root.join(marker))
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                {
                    out.name = v["name"]
                        .as_str()
                        .filter(|n| !n.trim().is_empty())
                        .map(str::to_string);
                    for (dep, label) in [
                        ("vite", "Vite"),
                        ("next", "Next.js"),
                        ("nuxt", "Nuxt"),
                        ("@angular/core", "Angular"),
                        ("react", "React"),
                        ("vue", "Vue"),
                    ] {
                        if v["dependencies"].get(dep).is_some()
                            || v["devDependencies"].get(dep).is_some()
                        {
                            out.frameworks.push(label.into());
                        }
                    }
                }
                break;
            }
            "Cargo.toml" | "pyproject.toml" => {
                out.ecosystem = Some(
                    if marker == "Cargo.toml" {
                        "Rust"
                    } else {
                        "Python"
                    }
                    .into(),
                );
                if let Some(v) = read_small(&root.join(marker))
                    .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
                {
                    let section = if marker == "Cargo.toml" {
                        "package"
                    } else {
                        "project"
                    };
                    out.name = v
                        .get(section)
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .or_else(|| v.get("tool")?.get("poetry")?.get("name")?.as_str())
                        .map(str::to_string);
                    let deps = v
                        .get("project")
                        .and_then(|p| p.get("dependencies"))
                        .map(ToString::to_string)
                        .unwrap_or_default()
                        + &v.get("tool")
                            .and_then(|p| p.get("poetry"))
                            .and_then(|p| p.get("dependencies"))
                            .map(ToString::to_string)
                            .unwrap_or_default();
                    for (dep, label) in [
                        ("django", "Django"),
                        ("flask", "Flask"),
                        ("fastapi", "FastAPI"),
                    ] {
                        if deps.to_lowercase().contains(dep) {
                            out.frameworks.push(label.into());
                        }
                    }
                }
                break;
            }
            "requirements.txt" => {
                out.ecosystem = Some("Python".into());
            }
            "go.mod" => {
                out.ecosystem = Some("Go".into());
                out.name = read_small(&root.join(marker)).and_then(|s| {
                    s.lines().find_map(|l| {
                        l.strip_prefix("module ")
                            .map(|n| n.trim().rsplit('/').next().unwrap_or(n).to_string())
                    })
                });
            }
            "pom.xml" | "build.gradle" | "build.gradle.kts" => out.ecosystem = Some("Java".into()),
            "composer.json" => {
                out.ecosystem = Some("PHP".into());
                out.name = read_small(&root.join(marker))
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v["name"].as_str().map(str::to_string));
            }
            "Gemfile" => out.ecosystem = Some("Ruby".into()),
            n if n.ends_with(".sln") || n.ends_with(".csproj") => {
                out.ecosystem = Some(".NET".into())
            }
            _ if out.ecosystem.is_none() => out.ecosystem = Some("Docker Compose".into()),
            _ => {}
        }
    }
    out
}
pub fn display_name(name: &str) -> String {
    let name = name
        .trim()
        .trim_start_matches('@')
        .rsplit('/')
        .next()
        .unwrap_or(name);
    name.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}
