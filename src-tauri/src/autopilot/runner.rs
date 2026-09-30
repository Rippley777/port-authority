use super::{capture::Capture, detector::clean_output};
use std::{
    io::{Read, Write},
    process::Child,
    sync::{Arc, Mutex},
    thread,
};
use sysinfo::{Pid, ProcessesToUpdate, System};
pub const OUTPUT_LIMIT: usize = 32768;
pub type Output = Arc<Mutex<Vec<u8>>>;

pub fn pump(
    mut input: impl Read + Send + 'static,
    output: Output,
    terminal: Option<bool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut bytes = [0u8; 4096];
        loop {
            let count = match input.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if let Some(stderr) = terminal {
                if stderr {
                    let _ = std::io::stderr().write_all(&bytes[..count]);
                    let _ = std::io::stderr().flush();
                } else {
                    let _ = std::io::stdout().write_all(&bytes[..count]);
                    let _ = std::io::stdout().flush();
                }
            }
            if let Ok(mut buffer) = output.lock() {
                buffer.extend_from_slice(&bytes[..count]);
                let excess = buffer.len().saturating_sub(OUTPUT_LIMIT);
                if excess > 0 {
                    buffer.drain(..excess);
                }
            }
        }
    })
}
pub fn output_text(output: &Output) -> String {
    output
        .lock()
        .map(|o| clean_output(&String::from_utf8_lossy(&o)))
        .unwrap_or_default()
}

pub struct ManagedRun {
    pub pid: u32,
    pub started_at: u64,
    pub capture: Capture,
    child: Mutex<Option<Child>>,
    detached_output: Option<std::path::PathBuf>,
    pub output: Output,
}
impl ManagedRun {
    pub fn spawn(capture: Capture) -> Result<Arc<Self>, String> {
        let mut child = capture
            .command()?
            .spawn()
            .map_err(|e| format!("Unable to retry the captured command: {e}"))?;
        let output = Arc::new(Mutex::new(Vec::new()));
        if let Some(stdout) = child.stdout.take() {
            pump(stdout, output.clone(), None);
        }
        if let Some(stderr) = child.stderr.take() {
            pump(stderr, output.clone(), None);
        }
        let pid = child.id();
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        let started_at = system
            .process(Pid::from_u32(pid))
            .map(|p| p.start_time())
            .unwrap_or(0);
        Ok(Arc::new(Self {
            pid,
            started_at,
            capture,
            child: Mutex::new(Some(child)),
            detached_output: None,
            output,
        }))
    }
    pub fn spawn_detached(
        capture: Capture,
        directory: &std::path::Path,
    ) -> Result<Arc<Self>, String> {
        let (root, path) = crate::recovery::executor::launch(capture.clone(), directory)?;
        Ok(Arc::new(Self {
            pid: root.pid,
            started_at: root.started_at,
            capture,
            child: Mutex::new(None),
            detached_output: Some(path),
            output: Arc::new(Mutex::new(Vec::new())),
        }))
    }
    pub fn output_text(&self) -> String {
        if let Some(path) = &self.detached_output {
            crate::recovery::executor::output(path)
        } else {
            crate::recovery::launch_context::redact(&output_text(&self.output), &self.capture)
        }
    }
    pub fn output_finished(&self) -> bool {
        self.detached_output.as_ref().is_none_or(|path| {
            crate::recovery::executor::output(path).contains("Command exited with")
        })
    }
    #[cfg(test)]
    pub fn stop_for_test(&self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            } else {
                let _ =
                    crate::process::controller::control(self.pid, Some(self.started_at), "force");
            }
        }
    }
    pub fn exit_status(&self) -> Option<String> {
        match self.child.lock() {
            Ok(mut child) => match child.as_mut() {
                None => (!crate::recovery::resolver::alive(crate::process::ProcessIdentity {
                    pid: self.pid,
                    started_at: self.started_at,
                }))
                .then(|| "The relaunched command exited. View output for details.".into()),
                Some(child) => match child.try_wait() {
                    Ok(Some(status)) => Some(format!("Command exited with {status}.")),
                    Ok(None) => None,
                    Err(e) => Some(format!("Unable to inspect the retried command: {e}")),
                },
            },
            Err(_) => Some("Unable to inspect the retried command.".into()),
        }
    }
    pub fn owns_pid(&self, pid: u32) -> bool {
        if self.started_at == 0 || self.exit_status().is_some() {
            return false;
        }
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]), true);
        if system
            .process(Pid::from_u32(self.pid))
            .is_none_or(|p| p.start_time() != self.started_at)
        {
            return false;
        }
        let mut current = Pid::from_u32(pid);
        for _ in 0..32 {
            if current.as_u32() == self.pid {
                return true;
            }
            system.refresh_processes(ProcessesToUpdate::Some(&[current]), true);
            let Some(parent) = system.process(current).and_then(|p| p.parent()) else {
                return false;
            };
            if parent == current || parent.as_u32() <= 1 {
                return false;
            }
            current = parent;
        }
        false
    }
}
