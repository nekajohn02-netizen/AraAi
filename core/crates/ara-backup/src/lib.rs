//! Encrypted local backup (export/import) of Ara's own data directory —
//! conversations, documents/search index, settings, permissions. A user-
//! held passphrase, no server, no cloud upload: this closes the "Document
//! Provider export/import" gap the blueprint names and
//! `docs/MASTER_CAPABILITY_REGISTRY.md` had flagged as `MISSING` under
//! "Backup and recovery".
//!
//! **Format** (before encryption): a flat sequence of entries, each
//! `[u32 path_len LE][path, UTF-8, forward-slash-separated, relative to the
//! data directory][u64 content_len LE][content bytes]`, repeated to the end
//! of the buffer. That whole buffer is encrypted with [`ara_crypto::seal`]
//! using a key derived from the user's passphrase
//! ([`ara_crypto::derive_key_from_passphrase`]); the salt it used is stored
//! in the archive's own small plaintext header (not secret — same role as
//! `seal`'s own embedded nonce).
//!
//! **On-disk archive layout:** `[magic "ARAB"(4)][version(1)][salt(16)]
//! [sealed payload]`.
//!
//! **Not included:** anything under a `models/` subdirectory — multi-
//! gigabyte GGUF files the user re-downloads from their official publisher
//! anyway (ADR-004). Callers pass `exclude_top_level` for this; backing
//! models up would make every export huge for no real benefit.
//!
//! **Threat model, stated plainly:** this protects the archive *file* at
//! rest (sitting in Downloads, on an SD card, in a cloud-synced folder)
//! with a passphrase-derived key. It does not protect against a
//! compromised device while Ara itself is running, and a forgotten
//! passphrase makes the archive unrecoverable — there is no backdoor and
//! no recovery mechanism, by design (same as a lost Keystore key, ADR-018).
//!
//! **Not done by this crate:** no Kotlin UI calls this yet (Storage Access
//! Framework file picker for choosing where to save/open the archive, a
//! passphrase entry screen, progress reporting for a large data directory)
//! — this is the tested core logic, same incremental pattern as
//! `ara-crypto`/`ara-smarthome` before their own UI wiring landed.

use ara_core_api::{AraError, AraResult};
use ara_crypto::{derive_key_from_passphrase, open, random_salt, seal, SALT_LEN};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAGIC: &[u8; 4] = b"ARAB";
const FORMAT_V1: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN;

/// Packs everything under `home_dir` (except any top-level entry named in
/// `exclude_top_level`) into one passphrase-encrypted archive.
pub fn export(home_dir: &Path, passphrase: &str, exclude_top_level: &[&str]) -> AraResult<Vec<u8>> {
    if passphrase.is_empty() {
        return Err(AraError::InvalidInput("a passphrase is required".into()));
    }
    let mut files = Vec::new();
    collect_files(home_dir, home_dir, exclude_top_level, &mut files)?;

    let mut payload = Vec::new();
    for (rel_path, abs_path) in files {
        let bytes = fs::read(&abs_path)?;
        let path_bytes = rel_path.as_bytes();
        payload.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(path_bytes);
        payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        payload.extend_from_slice(&bytes);
    }

    let salt = random_salt()?;
    let key = derive_key_from_passphrase(passphrase, &salt);
    let sealed = seal(&key, &payload)?;

    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(MAGIC);
    out.push(FORMAT_V1);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Restores an archive produced by [`export`] into `home_dir`. Returns the
/// number of files written. Rejects any entry whose path would escape
/// `home_dir` (`..`, an absolute path, …) instead of writing it — a
/// corrupted or maliciously crafted archive must never be able to write
/// outside the data directory, the same rule `ConversationStore` already
/// enforces for its own filenames.
pub fn import(archive: &[u8], passphrase: &str, home_dir: &Path) -> AraResult<usize> {
    if archive.len() < HEADER_LEN {
        return Err(AraError::InvalidInput("archive is too short".into()));
    }
    if &archive[0..4] != MAGIC {
        return Err(AraError::InvalidInput("not an Ara backup archive".into()));
    }
    if archive[4] != FORMAT_V1 {
        return Err(AraError::InvalidInput("unknown archive format version".into()));
    }
    let salt: [u8; SALT_LEN] = archive[5..HEADER_LEN]
        .try_into()
        .map_err(|_| AraError::InvalidInput("archive is too short".into()))?;
    let key = derive_key_from_passphrase(passphrase, &salt);
    let payload = open(&key, &archive[HEADER_LEN..])?;

    let mut count = 0usize;
    let mut i = 0usize;
    let corrupt = || AraError::InvalidInput("corrupted archive".into());
    while i < payload.len() {
        let path_len = read_u32(&payload, &mut i).ok_or_else(corrupt)? as usize;
        let rel_path = std::str::from_utf8(read_bytes(&payload, &mut i, path_len).ok_or_else(corrupt)?)
            .map_err(|_| corrupt())?;
        let content_len = read_u64(&payload, &mut i).ok_or_else(corrupt)? as usize;
        let content = read_bytes(&payload, &mut i, content_len).ok_or_else(corrupt)?;

        let safe_path = sanitize_relative_path(rel_path)?;
        let target = home_dir.join(&safe_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, content)?;
        count += 1;
    }
    Ok(count)
}

fn read_u32(buf: &[u8], i: &mut usize) -> Option<u32> {
    let bytes: [u8; 4] = buf.get(*i..*i + 4)?.try_into().ok()?;
    *i += 4;
    Some(u32::from_le_bytes(bytes))
}

fn read_u64(buf: &[u8], i: &mut usize) -> Option<u64> {
    let bytes: [u8; 8] = buf.get(*i..*i + 8)?.try_into().ok()?;
    *i += 8;
    Some(u64::from_le_bytes(bytes))
}

fn read_bytes<'a>(buf: &'a [u8], i: &mut usize, len: usize) -> Option<&'a [u8]> {
    let slice = buf.get(*i..*i + len)?;
    *i += len;
    Some(slice)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    exclude_top_level: &[&str],
    out: &mut Vec<(String, PathBuf)>,
) -> AraResult<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(()); // a missing/unreadable subdir just contributes nothing
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if dir == root {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if exclude_top_level.contains(&name) {
                    continue;
                }
            }
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, exclude_top_level, out)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| AraError::Internal("path outside data directory".into()))?;
            let rel_str = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel_str, path));
        }
    }
    Ok(())
}

/// Rejects anything that isn't a plain relative path within the data
/// directory: no `..`, no absolute path, no empty path.
fn sanitize_relative_path(rel_path: &str) -> AraResult<PathBuf> {
    let path = Path::new(rel_path);
    if rel_path.is_empty() || path.is_absolute() {
        return Err(AraError::InvalidInput("unsafe path in archive".into()));
    }
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            _ => return Err(AraError::InvalidInput("unsafe path in archive".into())),
        }
    }
    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, rel: &str, content: &[u8]) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    #[test]
    fn export_import_round_trip_preserves_files_and_structure() {
        let src = tempfile::tempdir().unwrap();
        write(src.path(), "conversations/a.json", b"{\"id\":\"a\"}");
        write(src.path(), "conversations/b.json", b"{\"id\":\"b\"}");
        write(src.path(), "settings.json", b"{\"context_tokens\":4096}");
        write(src.path(), "index.db", b"\x00sqlite-ish-bytes\x00");
        write(src.path(), "models/big.gguf", b"should not be backed up");

        let archive = export(src.path(), "a real passphrase", &["models"]).unwrap();
        assert!(
            !archive.windows(11).any(|w| w == b"sqlite-ish-"),
            "payload must be encrypted, not sitting in the archive in the clear"
        );

        let dst = tempfile::tempdir().unwrap();
        let count = import(&archive, "a real passphrase", dst.path()).unwrap();
        // conversations/a.json, conversations/b.json, settings.json, index.db
        // — models/big.gguf must NOT be among them.
        assert_eq!(count, 4, "the excluded models/ file must not be restored");

        assert_eq!(fs::read(dst.path().join("conversations/a.json")).unwrap(), b"{\"id\":\"a\"}");
        assert_eq!(fs::read(dst.path().join("conversations/b.json")).unwrap(), b"{\"id\":\"b\"}");
        assert_eq!(
            fs::read(dst.path().join("settings.json")).unwrap(),
            b"{\"context_tokens\":4096}"
        );
        assert!(!dst.path().join("models").exists());
    }

    #[test]
    fn wrong_passphrase_fails_to_restore() {
        let src = tempfile::tempdir().unwrap();
        write(src.path(), "settings.json", b"{}");
        let archive = export(src.path(), "right passphrase", &[]).unwrap();

        let dst = tempfile::tempdir().unwrap();
        let err = import(&archive, "wrong passphrase", dst.path()).unwrap_err();
        assert!(err.to_string().contains("decryption failed") || err.to_string().contains("wrong key"));
    }

    #[test]
    fn empty_passphrase_is_rejected_up_front() {
        let src = tempfile::tempdir().unwrap();
        assert!(export(src.path(), "", &[]).is_err());
    }

    #[test]
    fn not_an_archive_or_tampered_archive_is_rejected_cleanly() {
        let dst = tempfile::tempdir().unwrap();
        assert!(import(b"not an archive at all", "x", dst.path()).is_err());

        let src = tempfile::tempdir().unwrap();
        write(src.path(), "settings.json", b"{}");
        let mut archive = export(src.path(), "pass", &[]).unwrap();
        let last = archive.len() - 1;
        archive[last] ^= 1;
        assert!(import(&archive, "pass", dst.path()).is_err(), "GCM tag must catch tampering");
    }

    #[test]
    fn path_traversal_in_a_crafted_archive_is_rejected_not_written() {
        // Hand-build a minimal archive with a malicious path, bypassing
        // `export` (which only ever writes real relative paths) to prove
        // `import` itself refuses it rather than trusting the format.
        let evil_path = "../../etc/evil";
        let mut payload = Vec::new();
        payload.extend_from_slice(&(evil_path.len() as u32).to_le_bytes());
        payload.extend_from_slice(evil_path.as_bytes());
        payload.extend_from_slice(&4u64.to_le_bytes());
        payload.extend_from_slice(b"evil");

        let salt = ara_crypto::random_salt().unwrap();
        let key = ara_crypto::derive_key_from_passphrase("pass", &salt);
        let sealed = ara_crypto::seal(&key, &payload).unwrap();
        let mut archive = Vec::new();
        archive.extend_from_slice(MAGIC);
        archive.push(FORMAT_V1);
        archive.extend_from_slice(&salt);
        archive.extend_from_slice(&sealed);

        let dst = tempfile::tempdir().unwrap();
        let err = import(&archive, "pass", dst.path()).unwrap_err();
        assert!(err.to_string().contains("unsafe path"));
        assert!(!dst.path().parent().unwrap().join("etc").exists());
    }

    #[test]
    fn models_directory_is_never_read_when_excluded() {
        let src = tempfile::tempdir().unwrap();
        write(src.path(), "settings.json", b"{}");
        write(src.path(), "models/a.gguf", b"huge");
        write(src.path(), "models/b.gguf", b"also huge");
        let archive = export(src.path(), "pass", &["models"]).unwrap();
        let dst = tempfile::tempdir().unwrap();
        let count = import(&archive, "pass", dst.path()).unwrap();
        assert_eq!(count, 1, "only settings.json should be in the archive");
    }
}
