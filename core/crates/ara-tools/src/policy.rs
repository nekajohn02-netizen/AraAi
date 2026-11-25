//! User permission grants and the allow / ask / deny decision.

use crate::{Permission, SideEffect, ToolManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    Denied,
    AskEveryTime,
    /// Allowed without asking — still overridden to Ask for External effects.
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ask,
    Deny(String),
}

/// Per-permission grants, persisted by the app. Missing = Denied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PermissionStore {
    grants: BTreeMap<Permission, Grant>,
}

impl PermissionStore {
    pub fn set(&mut self, p: Permission, g: Grant) {
        self.grants.insert(p, g);
    }

    pub fn get(&self, p: Permission) -> Grant {
        self.grants.get(&p).copied().unwrap_or(Grant::Denied)
    }

    #[doc(hidden)]
    pub fn allow_all_for_tests() -> Self {
        let mut s = Self::default();
        use Permission::*;
        for p in [
            LaunchApps,
            ComposeMessages,
            ShareContent,
            PhoneDialer,
            Alarms,
            Calendar,
            Clipboard,
            DeviceControls,
            OpenWeb,
            ReadUserFiles,
            Memory,
        ] {
            s.set(p, Grant::Always);
        }
        s
    }

    pub fn decide(&self, m: &ToolManifest) -> Decision {
        let mut ask = false;
        for p in &m.permissions {
            match self.get(*p) {
                Grant::Denied => {
                    return Decision::Deny(format!("permission {p:?} not granted"));
                }
                Grant::AskEveryTime => ask = true,
                Grant::Always => {}
            }
        }
        // Anything that reaches other people always needs a human "yes".
        if ask || m.side_effect == SideEffect::External {
            Decision::Ask
        } else {
            Decision::Allow
        }
    }
}
