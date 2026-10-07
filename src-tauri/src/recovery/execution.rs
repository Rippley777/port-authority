//! Private durable execution data. This type is never returned by a Tauri command.
use super::{
    launch_context,
    models::{DisplayLaunchContext, LaunchKind, Source},
};
use crate::autopilot::capture::Capture;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentStrategy {
    CurrentWithCapturedToolchain,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchContext {
    pub schema_version: u32,
    pub id: String,
    pub executable: String,
    pub argv0: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub kind: LaunchKind,
    pub shell: Option<String>,
    pub source: Source,
    pub project_id: Option<String>,
    pub captured_at: u64,
    pub environment_strategy: EnvironmentStrategy,
    pub toolchain_environment: BTreeMap<String, String>,
    pub directory_identity: Option<(u64, u64)>,
}
fn directory_identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        path.metadata().ok().map(|m| (m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}
impl LaunchContext {
    pub fn from_capture(display: &DisplayLaunchContext, capture: &Capture) -> Option<Self> {
        // Sanitized arguments are never a persistence source. If a secret would
        // be masked, retain the original only in the existing private RAM capture.
        if launch_context::safe_argv(capture) != capture.argv
            || capture.argv.iter().any(|arg| arg.contains("••••"))
            || capture.executable.contains("••••")
            || capture.cwd.contains("••••")
        {
            return None;
        }
        Some(Self {
            schema_version: 2,
            id: display.id.clone(),
            executable: capture.executable.clone(),
            argv0: capture.argv.first()?.clone(),
            args: capture.argv[1..].to_vec(),
            working_directory: capture.cwd.clone(),
            kind: display.kind.clone(),
            shell: display.shell.clone(),
            source: display.source.clone(),
            project_id: display.project_id.clone(),
            captured_at: display.captured_at,
            environment_strategy: EnvironmentStrategy::CurrentWithCapturedToolchain,
            toolchain_environment: capture
                .env
                .iter()
                .filter(|(k, v)| {
                    launch_context::public_env(k) && launch_context::redact(v, capture) == **v
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            directory_identity: directory_identity(Path::new(&capture.cwd)),
        })
    }
    pub fn capture(&self) -> Result<Capture, String> {
        if self.schema_version != 2
            || self.executable.contains("••••")
            || self.working_directory.contains("••••")
            || self
                .args
                .iter()
                .chain(std::iter::once(&self.argv0))
                .any(|a| a.contains("••••"))
        {
            return Err("Original launch command was not retained. Capture it again or configure a recovery command.".into());
        }
        let mut env: BTreeMap<String, String> = std::env::vars().collect();
        // Only the small explicit toolchain allowlist is durable. Credentials
        // come from the current environment/project configuration, never SQLite.
        env.extend(
            self.toolchain_environment
                .iter()
                .filter(|(k, _)| launch_context::public_env(k))
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        Ok(Capture {
            executable: self.executable.clone(),
            argv: std::iter::once(self.argv0.clone())
                .chain(self.args.clone())
                .collect(),
            cwd: self.working_directory.clone(),
            env,
            error: String::new(),
            exit_code: 1,
        })
    }
    pub fn validate_directory(&self) -> Result<(), String> {
        if self.directory_identity.is_some()
            && directory_identity(Path::new(&self.working_directory)) != self.directory_identity
        {
            return Err("The original project directory was moved or replaced. Update its recovery command before running it.".into());
        }
        Ok(())
    }
}
