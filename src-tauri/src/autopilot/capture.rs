use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
/// Secret-bearing context: never Debug, never returned through Tauri, never saved
/// to disk. Serialization is used only on the private same-user Unix socket.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capture {
    pub argv: Vec<String>,
    pub executable: String,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub error: String,
    pub exit_code: i32,
}
impl Capture {
    pub fn validate(&self) -> Result<(), String> {
        if self.argv.is_empty()
            || self.argv.len() > 256
            || self.argv.iter().map(String::len).sum::<usize>() > 65536
        {
            return Err("Command arguments are empty or too large.".into());
        }
        #[cfg(windows)]
        if Path::new(&self.executable)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        {
            return Err("Batch-file recovery requires an explicitly configured shell command; implicit cmd.exe evaluation is disabled.".into());
        }
        if self.exit_code == 0 {
            return Err("Only failed commands can be registered as conflicts.".into());
        }
        if !Path::new(&self.executable).is_absolute()
            || !Path::new(&self.executable).is_file()
            || !Path::new(&self.cwd).is_absolute()
            || !Path::new(&self.cwd).is_dir()
        {
            return Err(
                "The original executable or working directory is no longer available.".into(),
            );
        }
        if self
            .env
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>()
            > 262144
            || self
                .env
                .iter()
                .any(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0'))
            || self.argv.iter().any(|s| s.contains('\0'))
            || self.error.len() > 32768
        {
            return Err("Captured context exceeds limits or contains invalid data.".into());
        }
        Ok(())
    }
    pub fn command(&self) -> Result<Command, String> {
        crate::recovery::launch_context::command(self)
    }

    pub fn alternate(&self, port: u16) -> Option<Self> {
        let mut capture = self.clone();
        let first = Path::new(self.argv.first()?).file_name()?.to_str()?;
        let offset;
        let framework;
        if matches!(first, "vite" | "next") {
            framework = first.to_owned();
            offset = 1;
            if first == "next" && self.argv.get(1).map(String::as_str) != Some("dev") {
                return None;
            }
        } else if first == "npm" && self.argv.get(1).map(String::as_str) == Some("run") {
            let script_name = self.argv.get(2)?;
            let contents = std::fs::read(Path::new(&self.cwd).join("package.json")).ok()?;
            if contents.len() > 1_048_576 {
                return None;
            }
            let package: serde_json::Value = serde_json::from_slice(&contents).ok()?;
            let script = package.get("scripts")?.get(script_name)?.as_str()?;
            // Deliberately accept only simple known scripts; never rewrite shell
            // operators, quoting, workspace selectors, or unknown framework flags.
            let words: Vec<_> = script.split_whitespace().collect();
            if words == ["vite"] || words == ["vite", "dev"] {
                framework = "vite".into();
            } else if words == ["next", "dev"] {
                framework = "next".into();
            } else {
                return None;
            }
            if self.argv.len() > 3 && self.argv[3] != "--" {
                return None;
            }
            if capture.argv.len() == 3 {
                capture.argv.push("--".into());
            }
            offset = 4;
        } else {
            return None;
        }
        let mut found = false;
        let mut i = offset;
        while i < capture.argv.len() {
            let arg = &capture.argv[i];
            if arg == "--port" || (framework == "next" && arg == "-p") {
                if found || capture.argv.get(i + 1)?.parse::<u16>().ok().is_none() {
                    return None;
                }
                capture.argv[i + 1] = port.to_string();
                found = true;
                i += 2;
                continue;
            }
            if arg.starts_with("--port=") {
                if found
                    || arg
                        .trim_start_matches("--port=")
                        .parse::<u16>()
                        .ok()
                        .is_none()
                {
                    return None;
                }
                capture.argv[i] = format!("--port={port}");
                found = true;
            }
            i += 1;
        }
        if !found {
            capture.argv.extend(["--port".into(), port.to_string()]);
        }
        Some(capture)
    }
}
pub fn resolve_executable(program: &str, cwd: &Path) -> Option<PathBuf> {
    let candidate = if program.contains('/') {
        let p = PathBuf::from(program);
        if p.is_absolute() {
            p
        } else {
            cwd.join(p)
        }
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|p| {
                if p.is_absolute() {
                    p.join(program)
                } else {
                    cwd.join(p).join(program)
                }
            })
            .find(|p| is_executable(p))?
    };
    // Preserve the invocation path: resolving a virtualenv's Python symlink to
    // the system interpreter changes sys.prefix and can lose installed packages.
    is_executable(&candidate).then_some(candidate)
}
fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
