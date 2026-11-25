//! Model lifecycle: download (resumable), SHA-256 verification, storage
//! budget, duplicate prevention, safe deletion.
//!
//! Layout under the store root (app-private storage on Android):
//! - `<id>.gguf`       verified, installed model
//! - `<id>.gguf.part`  in-progress download (resumed with HTTP Range)
//! - `<id>.lock`       exclusive lock while a download runs (create_new)
//!
//! A model file only gets its final name after its checksum matched, so an
//! installed file is never partial. Catalog entries without a checksum are
//! refused.

use ara_core_api::{AraError, AraResult, CancellationToken};
use ara_model_registry::ModelSpec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHUNK: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InstallState {
    NotInstalled,
    /// A partial download of `bytes` exists and can be resumed.
    Partial {
        bytes: u64,
    },
    Installed,
}

/// User-controlled limits so models never fill the device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoragePolicy {
    /// Max total bytes of installed + in-progress models. `None` = no cap.
    pub max_total_bytes: Option<u64>,
    /// Free space that must remain on the volume after a download.
    pub min_free_after_bytes: u64,
}

impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            max_total_bytes: None,
            min_free_after_bytes: 1 << 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub downloaded: u64,
    pub total: u64,
}

pub struct ModelStore {
    root: PathBuf,
    agent: ureq::Agent,
    /// Only the unit tests' local HTTP server may use plain http.
    allow_http: bool,
}

/// Releases the per-model lock file on drop.
struct LockGuard(PathBuf);
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl ModelStore {
    pub fn open(root: impl Into<PathBuf>) -> AraResult<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .user_agent("ARA/0.1 (model-manager)")
            .build()
            .into();
        Ok(Self {
            root,
            agent,
            allow_http: false,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn model_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.gguf"))
    }
    fn part_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.gguf.part"))
    }
    fn lock_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.lock"))
    }

    pub fn state(&self, spec: &ModelSpec) -> InstallState {
        if self.model_path(&spec.id).is_file() {
            InstallState::Installed
        } else if let Ok(m) = fs::metadata(self.part_path(&spec.id)) {
            InstallState::Partial { bytes: m.len() }
        } else {
            InstallState::NotInstalled
        }
    }

    /// Ids of installed models (by file name).
    pub fn installed_ids(&self) -> AraResult<Vec<String>> {
        let mut ids = Vec::new();
        for e in fs::read_dir(&self.root)? {
            let name = e?.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_suffix(".gguf") {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Bytes used by installed models and partial downloads.
    pub fn used_bytes(&self) -> AraResult<u64> {
        let mut total = 0;
        for e in fs::read_dir(&self.root)? {
            let e = e?;
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(".gguf") || n.ends_with(".gguf.part") {
                total += e.metadata()?.len();
            }
        }
        Ok(total)
    }

    fn check_space(&self, spec: &ModelSpec, already: u64, policy: &StoragePolicy) -> AraResult<()> {
        let remaining = spec.file_size_bytes.saturating_sub(already);
        if let Some(cap) = policy.max_total_bytes {
            let after = self.used_bytes()?.saturating_add(remaining);
            if after > cap {
                return Err(AraError::InsufficientResources(format!(
                    "model storage limit: would use {after} of {cap} bytes"
                )));
            }
        }
        let free = ara_device_profiler::free_storage_bytes(&self.root)?;
        if free < remaining.saturating_add(policy.min_free_after_bytes) {
            return Err(AraError::InsufficientResources(format!(
                "not enough free storage: need {} + reserve, have {free}",
                remaining
            )));
        }
        Ok(())
    }

    /// Downloads (or resumes) and verifies a model. Returns the installed path.
    pub fn download(
        &self,
        spec: &ModelSpec,
        policy: &StoragePolicy,
        cancel: &CancellationToken,
        on_progress: &mut dyn FnMut(Progress),
    ) -> AraResult<PathBuf> {
        let expected = spec
            .sha256
            .as_deref()
            .ok_or_else(|| AraError::InvalidInput("model has no verified checksum".into()))?;
        if !spec.source_url.starts_with("https://") && !self.allow_http {
            return Err(AraError::InvalidInput("source must be https".into()));
        }
        let final_path = self.model_path(&spec.id);
        if final_path.is_file() {
            return Ok(final_path);
        }
        let lock = self.lock_path(&spec.id);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|_| AraError::InvalidInput("download already in progress".into()))?;
        let _guard = LockGuard(lock);

        let part = self.part_path(&spec.id);
        let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > spec.file_size_bytes {
            fs::remove_file(&part)?;
            have = 0;
        }
        self.check_space(spec, have, policy)?;

        // Hash what we already have so the final digest covers the whole file.
        let mut hasher = Sha256::new();
        if have > 0 {
            let mut f = File::open(&part)?;
            let mut buf = vec![0u8; CHUNK];
            let mut left = have;
            while left > 0 {
                cancel.check()?;
                let n = f.read(&mut buf[..CHUNK.min(left as usize)])?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                left -= n as u64;
            }
        }

        if have < spec.file_size_bytes {
            let mut req = self.agent.get(&spec.source_url);
            if have > 0 {
                req = req.header("Range", &format!("bytes={have}-"));
            }
            let resp = req
                .call()
                .map_err(|e| AraError::Io(format!("download request failed: {e}")))?;
            let status = resp.status().as_u16();
            let mut file = OpenOptions::new().create(true).append(true).open(&part)?;
            if have > 0 && status == 200 {
                // Server ignored Range: start over.
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                have = 0;
                hasher = Sha256::new();
            } else if have > 0 && status != 206 {
                return Err(AraError::Io(format!("unexpected HTTP status {status}")));
            }
            let mut body = resp.into_body().into_reader();
            let mut buf = vec![0u8; CHUNK];
            let mut done = have;
            on_progress(Progress {
                downloaded: done,
                total: spec.file_size_bytes,
            });
            loop {
                if cancel.is_cancelled() {
                    file.flush()?;
                    return Err(AraError::Cancelled); // .part kept for resume
                }
                let n = body
                    .read(&mut buf)
                    .map_err(|e| AraError::Io(format!("download interrupted: {}", e.kind())))?;
                if n == 0 {
                    break;
                }
                if done + n as u64 > spec.file_size_bytes {
                    drop(file);
                    fs::remove_file(&part)?;
                    return Err(AraError::Io("server sent more data than expected".into()));
                }
                file.write_all(&buf[..n])?;
                hasher.update(&buf[..n]);
                done += n as u64;
                on_progress(Progress {
                    downloaded: done,
                    total: spec.file_size_bytes,
                });
            }
            file.sync_all()?;
            if done < spec.file_size_bytes {
                return Err(AraError::Io(format!(
                    "download incomplete ({done}/{} bytes); can be resumed",
                    spec.file_size_bytes
                )));
            }
        }

        let digest = to_hex(&hasher.finalize());
        if digest != expected {
            fs::remove_file(&part)?;
            return Err(AraError::InvalidInput(
                "checksum mismatch: corrupt download was deleted".into(),
            ));
        }
        fs::rename(&part, &final_path)?;
        Ok(final_path)
    }

    /// Re-hashes an installed model (e.g. after a crash or on user request).
    pub fn verify(&self, spec: &ModelSpec, cancel: &CancellationToken) -> AraResult<bool> {
        let expected = spec
            .sha256
            .as_deref()
            .ok_or_else(|| AraError::InvalidInput("model has no verified checksum".into()))?;
        let mut f = File::open(self.model_path(&spec.id))
            .map_err(|_| AraError::NotFound("model not installed".into()))?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            cancel.check()?;
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(to_hex(&h.finalize()) == expected)
    }

    /// Deletes a model and any partial download. Refuses while `in_use`.
    pub fn delete(&self, id: &str, in_use: bool) -> AraResult<()> {
        if in_use {
            return Err(AraError::InvalidInput("model is currently loaded".into()));
        }
        if self.lock_path(id).exists() {
            return Err(AraError::InvalidInput("download in progress".into()));
        }
        for p in [self.model_path(id), self.part_path(id)] {
            match fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_model_registry::fixtures::spec;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Minimal HTTP/1.1 server with Range support. `cut` truncates the first
    /// response after that many bytes to simulate a dropped connection.
    fn serve(data: Vec<u8>, cut: Option<usize>, honor_range: bool) -> (String, Arc<AtomicUsize>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/m.gguf", l.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let mut s = stream.unwrap();
                let n = h.fetch_add(1, Ordering::SeqCst);
                let mut start = 0usize;
                let mut r = BufReader::new(s.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        start = v.trim().trim_end_matches('-').parse().unwrap();
                    }
                }
                if !honor_range {
                    start = 0;
                }
                let body = &data[start..];
                let status = if start > 0 {
                    "206 Partial Content"
                } else {
                    "200 OK"
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let send = match (n, cut) {
                    (0, Some(c)) => &body[..c],
                    _ => body,
                };
                let _ = s.write_all(send);
            }
        });
        (url, hits)
    }

    impl ModelStore {
        fn open_test(root: &Path) -> Self {
            let mut s = ModelStore::open(root).unwrap();
            s.allow_http = true;
            s
        }
    }

    fn data() -> Vec<u8> {
        (0..300_000u32).map(|i| (i % 251) as u8).collect()
    }

    fn spec_for(url: &str, d: &[u8]) -> ModelSpec {
        let mut s = spec("m", 0.1, 1);
        s.file_size_bytes = d.len() as u64;
        s.source_url = url.to_string();
        s.sha256 = Some(to_hex(&Sha256::digest(d)));
        s
    }

    fn policy() -> StoragePolicy {
        StoragePolicy {
            max_total_bytes: None,
            min_free_after_bytes: 0,
        }
    }

    #[test]
    fn downloads_and_verifies() {
        let d = data();
        let (url, _) = serve(d.clone(), None, true);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        let s = spec_for(&url, &d);
        let mut last = None;
        let p = store
            .download(&s, &policy(), &CancellationToken::new(), &mut |pr| {
                last = Some(pr)
            })
            .unwrap();
        assert_eq!(fs::read(&p).unwrap(), d);
        assert_eq!(last.unwrap().downloaded, d.len() as u64);
        assert_eq!(store.state(&s), InstallState::Installed);
        assert!(store.verify(&s, &CancellationToken::new()).unwrap());
        assert_eq!(store.installed_ids().unwrap(), vec!["m".to_string()]);
    }

    #[test]
    fn resumes_after_interrupted_connection() {
        let d = data();
        let (url, hits) = serve(d.clone(), Some(100_000), true);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        let s = spec_for(&url, &d);
        let err = store
            .download(&s, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap_err();
        assert!(matches!(err, AraError::Io(_)));
        assert!(matches!(store.state(&s), InstallState::Partial { bytes } if bytes > 0));
        store
            .download(&s, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap();
        assert_eq!(fs::read(store.model_path("m")).unwrap(), d);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn restarts_when_server_ignores_range() {
        let d = data();
        let (url, _) = serve(d.clone(), Some(50_000), false);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        let s = spec_for(&url, &d);
        let _ = store.download(&s, &policy(), &CancellationToken::new(), &mut |_| {});
        store
            .download(&s, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap();
        assert_eq!(fs::read(store.model_path("m")).unwrap(), d);
    }

    #[test]
    fn corrupt_download_is_deleted() {
        let d = data();
        let (url, _) = serve(d.clone(), None, true);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        let mut s = spec_for(&url, &d);
        s.sha256 = Some("0".repeat(64));
        let err = store
            .download(&s, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));
        assert_eq!(store.state(&s), InstallState::NotInstalled);
    }

    #[test]
    fn refuses_unverified_duplicate_and_over_budget() {
        let d = data();
        let (url, _) = serve(d.clone(), None, true);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());

        let mut http = spec_for(&url, &d);
        http.id = "plain".into();
        let strict = ModelStore::open(dir.path()).unwrap();
        assert!(strict
            .download(&http, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap_err()
            .to_string()
            .contains("https"));

        let mut unverified = spec_for(&url, &d);
        unverified.sha256 = None;
        assert!(store
            .download(
                &unverified,
                &policy(),
                &CancellationToken::new(),
                &mut |_| {}
            )
            .is_err());

        let s = spec_for(&url, &d);
        fs::write(store.lock_path("m"), b"").unwrap();
        let err = store
            .download(&s, &policy(), &CancellationToken::new(), &mut |_| {})
            .unwrap_err();
        assert!(err.to_string().contains("already in progress"));
        fs::remove_file(store.lock_path("m")).unwrap();

        let capped = StoragePolicy {
            max_total_bytes: Some(1000),
            min_free_after_bytes: 0,
        };
        let err = store
            .download(&s, &capped, &CancellationToken::new(), &mut |_| {})
            .unwrap_err();
        assert!(matches!(err, AraError::InsufficientResources(_)));
    }

    #[test]
    fn cancellation_keeps_partial_for_resume() {
        let d = data();
        let (url, _) = serve(d.clone(), None, true);
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        let s = spec_for(&url, &d);
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        let err = store
            .download(&s, &policy(), &cancel, &mut |p| {
                if p.downloaded > 0 {
                    c2.cancel()
                }
            })
            .unwrap_err();
        assert_eq!(err, AraError::Cancelled);
        assert!(matches!(store.state(&s), InstallState::Partial { .. }));
        assert!(!store.lock_path("m").exists(), "lock released");
    }

    #[test]
    fn delete_refuses_model_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::open_test(dir.path());
        fs::write(store.model_path("m"), b"x").unwrap();
        assert!(store.delete("m", true).is_err());
        assert!(store.model_path("m").exists());
        store.delete("m", false).unwrap();
        assert!(!store.model_path("m").exists());
        store.delete("m", false).unwrap(); // idempotent
    }
}
