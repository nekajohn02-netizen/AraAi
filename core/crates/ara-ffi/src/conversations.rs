//! Local conversation storage: one JSON file per conversation in app-private
//! storage, written atomically (tmp + rename). Never uploaded. A SQLite store
//! with full-text search replaces this in the search stage (G5); the API stays.
//!
//! **Encryption at rest (T1.8, ADR-018):** `open_with_key` makes every
//! conversation file AES-256-GCM-sealed on disk (`ara_crypto::seal`/`open`).
//! `open` (no key) keeps the original plaintext behaviour unchanged — a
//! caller that does not opt in sees no behaviour change at all. The two
//! modes are *not* interchangeable on the same directory: an encrypted
//! store cannot read plaintext files or vice versa (by design — silently
//! treating one as the other would be a worse failure than a clear error).
//! Still TODO: `ara-ffi` wires a *real* Keystore-unwrapped key through from
//! Kotlin before this is used for real — today nothing calls
//! `open_with_key` outside this module's own tests, same honesty as
//! `NetworkPolicy`'s `TODO(T1.7)` in `ara-cloud`.

use ara_core_api::{AraError, AraResult};
use ara_inference::ChatMessage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    /// Specialist mode id (see ara_tools::personas). Missing = "general".
    #[serde(default = "general")]
    pub persona: String,
    pub messages: Vec<ChatMessage>,
}

fn general() -> String {
    "general".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_ms: u64,
    pub message_count: usize,
    pub persona: String,
}

pub struct ConversationStore {
    dir: PathBuf,
    key: Option<ara_crypto::Key>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn valid_id(id: &str) -> AraResult<()> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AraError::InvalidInput("bad conversation id".into()));
    }
    Ok(())
}

impl ConversationStore {
    pub fn open(dir: impl Into<PathBuf>) -> AraResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir, key: None })
    }

    /// Same as `open`, but every conversation file is AES-256-GCM-sealed
    /// with `key` (T1.8). `key` is raw key material already unwrapped from
    /// an Android-Keystore-held KEK — this store never talks to Keystore
    /// itself (see module docs and ADR-018).
    pub fn open_with_key(dir: impl Into<PathBuf>, key: ara_crypto::Key) -> AraResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            key: Some(key),
        })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn write(&self, c: &Conversation) -> AraResult<()> {
        let tmp = self.dir.join(format!("{}.json.tmp", c.id));
        let plain = serde_json::to_vec(c).map_err(|e| AraError::Internal(e.to_string()))?;
        let bytes = match &self.key {
            Some(k) => ara_crypto::seal(k, &plain)?,
            None => plain,
        };
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, self.path(&c.id))?;
        Ok(())
    }

    fn read(&self, p: &Path) -> Option<Conversation> {
        let bytes = fs::read(p).ok()?;
        let plain = match &self.key {
            Some(k) => ara_crypto::open(k, &bytes).ok()?,
            None => bytes,
        };
        serde_json::from_slice(&plain).ok()
    }

    pub fn create(&self, title: &str, persona: &str) -> AraResult<Conversation> {
        let t = now_ms();
        // Time + counter: unique without a random source.
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let c = Conversation {
            id: format!("{t:x}-{seq:x}"),
            title: clean_title(title),
            created_ms: t,
            updated_ms: t,
            persona: persona.to_string(),
            messages: vec![],
        };
        self.write(&c)?;
        Ok(c)
    }

    pub fn get(&self, id: &str) -> AraResult<Conversation> {
        valid_id(id)?;
        if !self.path(id).exists() {
            return Err(AraError::NotFound("conversation".into()));
        }
        self.read(&self.path(id))
            .ok_or_else(|| AraError::InvalidInput("conversation file is corrupt".into()))
    }

    pub fn append(&self, id: &str, msgs: &[ChatMessage]) -> AraResult<()> {
        let mut c = self.get(id)?;
        c.messages.extend_from_slice(msgs);
        c.updated_ms = now_ms();
        self.write(&c)
    }

    pub fn rename(&self, id: &str, title: &str) -> AraResult<()> {
        let mut c = self.get(id)?;
        c.title = clean_title(title);
        c.updated_ms = now_ms();
        self.write(&c)
    }

    pub fn delete(&self, id: &str) -> AraResult<()> {
        valid_id(id)?;
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Newest first. Corrupt files are skipped, not fatal.
    pub fn list(&self) -> AraResult<Vec<ConversationSummary>> {
        let mut out = Vec::new();
        for e in fs::read_dir(&self.dir)? {
            let p = e?.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Some(c) = self.read(&p) {
                out.push(ConversationSummary {
                    message_count: c.messages.len(),
                    persona: c.persona.clone(),
                    id: c.id,
                    title: c.title,
                    updated_ms: c.updated_ms,
                });
            }
        }
        out.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms).then(a.id.cmp(&b.id)));
        Ok(out)
    }
}

fn clean_title(t: &str) -> String {
    let t: String = t.chars().filter(|c| !c.is_control()).take(80).collect();
    let t = t.trim();
    if t.is_empty() {
        "محادثة جديدة".to_string()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl ConversationStore {
        fn create_t(&self, title: &str) -> AraResult<Conversation> {
            self.create(title, "general")
        }
    }
    use ara_inference::Role;

    #[test]
    fn create_append_rename_list_delete() {
        let d = tempfile::tempdir().unwrap();
        let s = ConversationStore::open(d.path()).unwrap();
        let a = s.create_t("  ").unwrap();
        assert_eq!(a.title, "محادثة جديدة");
        let b = s.create_t("رحلة\nالغردقة").unwrap();
        assert_eq!(b.title, "رحلةالغردقة");
        s.append(&a.id, &[ChatMessage::new(Role::User, "أهلا")])
            .unwrap();
        s.rename(&a.id, "تحية").unwrap();
        let l = s.list().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, a.id, "most recently updated first");
        assert_eq!(l[0].message_count, 1);
        assert_eq!(s.get(&a.id).unwrap().title, "تحية");
        s.delete(&a.id).unwrap();
        assert!(s.get(&a.id).is_err());
        s.delete(&a.id).unwrap();
    }

    #[test]
    fn rejects_path_traversal_and_survives_corruption() {
        let d = tempfile::tempdir().unwrap();
        let s = ConversationStore::open(d.path()).unwrap();
        assert!(s.get("../../etc/passwd").is_err());
        assert!(s.delete("../x").is_err());
        fs::write(d.path().join("broken.json"), b"{not json").unwrap();
        s.create_t("ok").unwrap();
        assert_eq!(s.list().unwrap().len(), 1);
    }

    // T1.8 (ADR-018): conversations are unreadable on disk without the key.
    #[test]
    fn encrypted_store_round_trips_and_file_is_not_plaintext_on_disk() {
        let d = tempfile::tempdir().unwrap();
        let key = ara_crypto::generate_key().unwrap();
        let s = ConversationStore::open_with_key(d.path(), key).unwrap();
        let c = s.create("خطة السفر", "general").unwrap();
        s.append(&c.id, &[ChatMessage::new(Role::User, "سر لا يجب أن يظهر في الملف")])
            .unwrap();

        let on_disk = fs::read(d.path().join(format!("{}.json", c.id))).unwrap();
        let on_disk_text = String::from_utf8_lossy(&on_disk);
        assert!(
            !on_disk_text.contains("سر لا يجب أن يظهر في الملف"),
            "raw file bytes must not contain the plaintext message"
        );
        assert!(!on_disk_text.contains("خطة السفر"));

        // Opening with the real key recovers it exactly.
        let reopened = ConversationStore::open_with_key(d.path(), key).unwrap();
        let got = reopened.get(&c.id).unwrap();
        assert_eq!(got.title, "خطة السفر");
        assert_eq!(got.messages.len(), 1);
    }

    #[test]
    fn encrypted_store_rejects_the_wrong_key() {
        let d = tempfile::tempdir().unwrap();
        let key = ara_crypto::generate_key().unwrap();
        let other_key = ara_crypto::generate_key().unwrap();
        let s = ConversationStore::open_with_key(d.path(), key).unwrap();
        let c = s.create_t("ok").unwrap();

        let wrong = ConversationStore::open_with_key(d.path(), other_key).unwrap();
        assert!(
            wrong.get(&c.id).is_err(),
            "the wrong key must not be able to read a storage file; a data-recovery \
             test like this is exactly what T1.8's acceptance criterion asks for"
        );
    }

    #[test]
    fn plaintext_and_encrypted_stores_cannot_silently_read_each_other() {
        let d = tempfile::tempdir().unwrap();
        let plain = ConversationStore::open(d.path()).unwrap();
        let c = plain.create_t("plain").unwrap();

        let key = ara_crypto::generate_key().unwrap();
        let encrypted = ConversationStore::open_with_key(d.path(), key).unwrap();
        assert!(
            encrypted.get(&c.id).is_err(),
            "a plaintext file must not be misread as a valid encrypted one"
        );
    }
}
