//! Shared types used by every ARA core module.
//!
//! Kept deliberately small: errors, request ids, streaming events and a
//! cooperative cancellation token. Nothing here performs I/O.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Error type shared across ARA crates. Messages must never contain user
/// content (conversation text, file contents, API keys) because errors may be
/// surfaced to logs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum AraError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("insufficient resources: {0}")]
    InsufficientResources(String),
    #[error("unsupported on this device: {0}")]
    Unsupported(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AraError {
    fn from(e: std::io::Error) -> Self {
        AraError::Io(e.kind().to_string())
    }
}

pub type AraResult<T> = Result<T, AraError>;

/// Monotonic per-process request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestId(pub u64);

impl RequestId {
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        RequestId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Where a piece of work actually ran. Shown to the user in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSource {
    Local,
    Cloud,
}

/// Events emitted by long-running operations (generation, downloads, indexing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    Started {
        request: RequestId,
        source: ExecutionSource,
    },
    Token {
        request: RequestId,
        text: String,
    },
    Progress {
        request: RequestId,
        done: u64,
        total: Option<u64>,
    },
    Finished {
        request: RequestId,
    },
    Failed {
        request: RequestId,
        error: AraError,
    },
    Cancelled {
        request: RequestId,
    },
}

/// Cooperative cancellation token. Cloning shares the same flag.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Convenience for loops: returns `Err(Cancelled)` once cancelled.
    pub fn check(&self) -> AraResult<()> {
        if self.is_cancelled() {
            Err(AraError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_are_unique_and_increasing() {
        let a = RequestId::next();
        let b = RequestId::next();
        assert!(b > a);
    }

    #[test]
    fn cancellation_is_shared_between_clones() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        assert!(t.check().is_ok());
        t2.cancel();
        assert!(t.is_cancelled());
        assert_eq!(t.check(), Err(AraError::Cancelled));
    }

    #[test]
    fn cancellation_crosses_threads() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        std::thread::spawn(move || t2.cancel()).join().unwrap();
        assert!(t.is_cancelled());
    }

    #[test]
    fn io_error_does_not_leak_message() {
        let e: AraError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "/secret/path/notes.txt").into();
        assert!(!e.to_string().contains("secret"));
    }

    #[test]
    fn events_serialize_with_stable_tags() {
        let ev = CoreEvent::Failed {
            request: RequestId(7),
            error: AraError::Cancelled,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"type\":\"failed\""), "{json}");
        let back: CoreEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
    }
}
