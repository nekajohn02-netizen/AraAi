//! Local conversation storage: one JSON file per conversation in app-private
//! storage, written atomically (tmp + rename). Never uploaded. A SQLite store
//! with full-text search replaces this in the search stage (G5); the API stays.

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
        Ok(Self { dir })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn write(&self, c: &Conversation) -> AraResult<()> {
        let tmp = self.dir.join(format!("{}.json.tmp", c.id));
        fs::write(
            &tmp,
            serde_json::to_vec(c).map_err(|e| AraError::Internal(e.to_string()))?,
        )?;
        fs::rename(&tmp, self.path(&c.id))?;
        Ok(())
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
        let bytes =
            fs::read(self.path(id)).map_err(|_| AraError::NotFound("conversation".into()))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AraError::InvalidInput("conversation file is corrupt".into()))
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
            if let Some(c) = read(&p) {
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

fn read(p: &Path) -> Option<Conversation> {
    serde_json::from_slice(&fs::read(p).ok()?).ok()
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
}
