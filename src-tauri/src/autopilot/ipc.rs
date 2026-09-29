//! Same-user, bounded, local-only capture transport. Environments never touch disk.
use super::{capture::Capture, engine::Autopilot};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
const LIMIT: u64 = 524288;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    capture: Capture,
}
#[derive(Serialize, Deserialize)]
struct Reply {
    id: Option<String>,
    error: Option<String>,
}
pub fn socket_path() -> Result<PathBuf, String> {
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if uid == 0 {
        return Err("Shell capture is disabled for root. Run Port Authority and your development command as a normal user.".into());
    }
    let dir = PathBuf::from(format!("/tmp/port-authority-{uid}"));
    match fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(format!("Unable to create private capture directory: {e}")),
    }
    let meta = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
        return Err("The capture directory is not a private, same-user directory. Integration was disabled.".into());
    }
    Ok(dir.join("autopilot.sock"))
}
fn same_peer(stream: &UnixStream) -> bool {
    // SAFETY: valid connected descriptor, initialized credential buffers, exact sizes.
    unsafe {
        #[cfg(target_os = "macos")]
        {
            let mut uid = 0;
            let mut gid = 0;
            libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) == 0 && uid == libc::geteuid()
        }
        #[cfg(target_os = "linux")]
        {
            let mut credential: libc::ucred = std::mem::zeroed();
            let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credential as *mut libc::ucred).cast(),
                &mut size,
            ) == 0
                && credential.uid == libc::geteuid()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            false
        }
    }
}
pub fn start(engine: Arc<Autopilot>) -> Result<(), String> {
    start_at(engine, socket_path()?)
}
fn start_at(engine: Arc<Autopilot>, path: PathBuf) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(&path) {
        // SAFETY: geteuid has no preconditions.
        if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
            return Err("The capture socket path is not owned by this user.".into());
        }
        match UnixStream::connect(&path) {
            Ok(_)=>return Err("Another Port Authority instance is handling shell captures. Close it before enabling this instance.".into()),
            Err(e) if matches!(e.kind(),std::io::ErrorKind::ConnectionRefused|std::io::ErrorKind::NotFound)=>fs::remove_file(&path).map_err(|e|e.to_string())?,
            Err(e)=>return Err(format!("Unable to inspect the existing capture socket: {e}")),
        }
    }
    let listener =
        UnixListener::bind(&path).map_err(|e| format!("Unable to start shell capture: {e}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if !same_peer(&stream) {
                continue;
            }
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let result = (|| {
                let mut bytes = Vec::new();
                (&mut stream)
                    .take(LIMIT + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Incomplete capture request".to_owned())?;
                if bytes.len() as u64 > LIMIT {
                    return Err("Capture request exceeds the size limit".into());
                }
                let request: Envelope = serde_json::from_slice(&bytes)
                    .map_err(|_| "Invalid capture request".to_owned())?;
                if request.version != 1 {
                    return Err("Unsupported shell integration version".into());
                }
                engine.register(request.capture)
            })();
            let reply = match result {
                Ok(id) => Reply {
                    id: Some(id),
                    error: None,
                },
                Err(error) => Reply {
                    id: None,
                    error: Some(error),
                },
            };
            if let Ok(bytes) = serde_json::to_vec(&reply) {
                let _ = stream.write_all(&bytes);
            }
        }
    });
    Ok(())
}
pub fn submit(capture: Capture) -> Result<String, String> {
    submit_at(capture, socket_path()?)
}
fn submit_at(capture: Capture, path: PathBuf) -> Result<String, String> {
    let mut stream = UnixStream::connect(path).map_err(|_| {
        "Open Port Authority and enable Conflict Autopilot to receive this conflict.".to_owned()
    })?;
    if !same_peer(&stream) {
        return Err("Capture server belongs to another user; context was not sent.".into());
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&Envelope {
        version: 1,
        capture,
    })
    .map_err(|_| "Unable to encode command context".to_owned())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Command context is too large to capture safely.".into());
    }
    stream.write_all(&bytes).map_err(|e| e.to_string())?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream
        .take(16384)
        .read_to_end(&mut response)
        .map_err(|e| e.to_string())?;
    let reply: Reply =
        serde_json::from_slice(&response).map_err(|_| "Invalid capture response".to_owned())?;
    reply
        .id
        .ok_or_else(|| reply.error.unwrap_or_else(|| "Capture was rejected".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::scanner::Scanner;
    use std::{collections::BTreeMap, sync::Mutex};
    #[test]
    fn private_transport_obeys_opt_in_limits_and_does_not_replace_a_live_server() {
        let dir = PathBuf::from("/tmp").join(format!("pa-ipc-test-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let path = dir.join("capture.sock");
        let engine = Arc::new(Autopilot::new(
            Arc::new(Mutex::new(Scanner::new())),
            String::new(),
        ));
        start_at(engine.clone(), path.clone()).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        assert!(start_at(engine.clone(), path.clone())
            .unwrap_err()
            .contains("Another Port Authority"));
        let capture = Capture {
            argv: vec!["sh".into(), "-c".into(), "exit 1".into()],
            executable: "/bin/sh".into(),
            cwd: dir.to_string_lossy().into_owned(),
            env: BTreeMap::from([("TOP_SECRET".into(), "transport-test-secret".into())]),
            error: "EADDRINUSE :::51899".into(),
            exit_code: 1,
        };
        assert!(submit_at(capture.clone(), path.clone())
            .unwrap_err()
            .contains("Enable Conflict Autopilot"));
        engine.enable(true).unwrap();
        let id = submit_at(capture.clone(), path.clone()).unwrap();
        let snapshot = engine.snapshot().unwrap();
        assert_eq!(snapshot.conflicts[0].id, id);
        let dto = serde_json::to_string(&snapshot).unwrap();
        assert!(!dto.contains("transport-test-secret"));
        assert!(!dto.contains("TOP_SECRET"));
        let mut invalid = capture;
        invalid.env.insert("TOO_BIG".into(), "x".repeat(300000));
        assert!(submit_at(invalid, path.clone())
            .unwrap_err()
            .contains("limits"));
        let mut connection = UnixStream::connect(&path).unwrap();
        connection.write_all(b"not json").unwrap();
        connection.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        connection.read_to_string(&mut response).unwrap();
        assert!(response.contains("Invalid capture request"));
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
