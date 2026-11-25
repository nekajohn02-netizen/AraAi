//! Sentinel: tells the user, with evidence, whether ARA's data and the
//! device look healthy. It reports findings; it never "fixes" anything
//! silently and never claims the device is "secure" — only that these
//! checks found nothing.
//!
//! Checks: (1) file-integrity baseline of ARA's own data (changed / new /
//! removed files); (2) listening TCP sockets visible to ARA (ARA itself never
//! listens, so any listener is suspicious); (3) device posture facts supplied
//! by Android (debuggable build, ADB, screen lock, root hints, …).

use ara_core_api::AraResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub title_ar: String,
    pub title_en: String,
    pub fix_ar: String,
    pub evidence: String,
}

/// Facts only Android can know. All optional: unknown is not "bad".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostSecurityFacts {
    pub app_debuggable: Option<bool>,
    pub adb_enabled: Option<bool>,
    pub developer_options: Option<bool>,
    pub screen_lock_set: Option<bool>,
    pub root_indicators: Vec<String>,
    pub security_patch: Option<String>,
    pub installed_from_store: Option<bool>,
    pub emulator: Option<bool>,
}

pub type Baseline = BTreeMap<String, String>;

fn sha_file(p: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Files that legitimately change while ARA runs (not integrity-checked).
fn volatile(name: &str) -> bool {
    name.ends_with(".json")
        || name.ends_with(".jsonl")
        || name.ends_with(".tmp")
        || name.ends_with(".part")
        || name.ends_with(".lock")
        || name.contains(".db")
}

/// Hashes immutable files (models) under `dir`, relative paths as keys.
pub fn snapshot(dir: &Path) -> AraResult<Baseline> {
    let mut out = Baseline::new();
    fn walk(root: &Path, dir: &Path, out: &mut Baseline) -> std::io::Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            let p = e.path();
            if e.file_type()?.is_dir() {
                walk(root, &p, out)?;
            } else if !volatile(&e.file_name().to_string_lossy()) {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
                out.insert(rel, sha_file(&p)?);
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut out)?;
    Ok(out)
}

pub fn integrity_findings(baseline: &Baseline, now: &Baseline) -> Vec<Finding> {
    let mut v = Vec::new();
    let f = |id: &str, sev, ar: &str, en: &str, fix: &str, ev: &str| Finding {
        id: id.into(),
        severity: sev,
        title_ar: ar.into(),
        title_en: en.into(),
        fix_ar: fix.into(),
        evidence: ev.into(),
    };
    for (k, h) in baseline {
        match now.get(k) {
            None => v.push(f(
                "file_removed",
                Severity::Low,
                "ملف نموذج اختفى",
                "A model file disappeared",
                "نزّل النموذج مرة أخرى.",
                k,
            )),
            Some(n) if n != h => v.push(f(
                "file_modified",
                Severity::High,
                "ملف تغيّر بدون تحديث",
                "A protected file changed",
                "احذف النموذج ونزّله من جديد؛ ممكن يكون تالف أو معدَّل.",
                k,
            )),
            _ => {}
        }
    }
    for k in now.keys().filter(|k| !baseline.contains_key(*k)) {
        v.push(f(
            "file_added",
            Severity::Medium,
            "ملف جديد ظهر في بيانات آرا",
            "Unexpected new file",
            "لو ما نزّلتهوش بنفسك، احذفه.",
            k,
        ));
    }
    v
}

/// Parses /proc/net/tcp(6) content, returning local ports in LISTEN state.
pub fn listening_ports(proc_net_tcp: &str) -> Vec<u16> {
    let mut ports: Vec<u16> = proc_net_tcp
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut c = l.split_whitespace();
            let local = c.nth(1)?;
            let state = c.nth(1)?; // after rem_address
            (state == "0A")
                .then(|| u16::from_str_radix(local.rsplit(':').next()?, 16).ok())
                .flatten()
        })
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

pub fn network_findings(ports: &[u16]) -> Vec<Finding> {
    ports
        .iter()
        .map(|p| Finding {
            id: "listening_port".into(),
            severity: Severity::Medium,
            title_ar: format!("منفذ شبكة مفتوح ({p})"),
            title_en: format!("Open listening port {p}"),
            fix_ar: "آرا لا يفتح منافذ. تحقق من التطبيقات المثبتة.".into(),
            evidence: p.to_string(),
        })
        .collect()
}

pub fn posture_findings(h: &HostSecurityFacts) -> Vec<Finding> {
    let mut v = Vec::new();
    let mut add = |id: &str, sev, ar: &str, en: &str, fix: &str, ev: String| {
        v.push(Finding {
            id: id.into(),
            severity: sev,
            title_ar: ar.into(),
            title_en: en.into(),
            fix_ar: fix.into(),
            evidence: ev,
        })
    };
    if h.screen_lock_set == Some(false) {
        add(
            "no_screen_lock",
            Severity::High,
            "الجهاز بدون قفل شاشة",
            "No screen lock",
            "فعّل بصمة أو PIN من إعدادات الأمان.",
            "screen_lock=false".into(),
        );
    }
    if !h.root_indicators.is_empty() {
        add(
            "root_detected",
            Severity::High,
            "علامات روت على الجهاز",
            "Root indicators found",
            "الروت يسمح لأي تطبيق بقراءة بيانات آرا. الأفضل جهاز غير مروّت للبيانات الحساسة.",
            h.root_indicators.join(", "),
        );
    }
    if h.app_debuggable == Some(true) {
        add(
            "debuggable_build",
            Severity::Medium,
            "نسخة قابلة للتصحيح (debug)",
            "Debuggable build",
            "استخدم نسخة الإصدار الرسمية.",
            "debuggable=true".into(),
        );
    }
    if h.adb_enabled == Some(true) {
        add(
            "adb_enabled",
            Severity::Low,
            "تصحيح USB مفعّل",
            "USB debugging is on",
            "أوقفه إن لم تكن تحتاجه.",
            "adb=true".into(),
        );
    }
    if h.installed_from_store == Some(false) {
        add(
            "sideloaded",
            Severity::Info,
            "مثبّت من خارج المتجر",
            "Installed outside a store",
            "تأكد من مصدر ملف التثبيت.",
            "installer=none".into(),
        );
    }
    if let Some(p) = &h.security_patch {
        // Patch strings are YYYY-MM-DD.
        if p.len() >= 7 && p.as_str() < "2025-06" {
            add(
                "old_patch",
                Severity::Medium,
                "تحديث الأمان قديم",
                "Old security patch",
                "حدّث النظام.",
                p.clone(),
            );
        }
    }
    if h.emulator == Some(true) {
        add(
            "emulator",
            Severity::Info,
            "يعمل على محاكي",
            "Running on an emulator",
            "",
            "emulator=true".into(),
        );
    }
    v
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// 0-100; starts at 100, minus weighted findings. A heuristic, not a guarantee.
    pub score: u32,
    pub findings: Vec<Finding>,
    pub checks_run: Vec<String>,
}

pub fn report(findings: Vec<Finding>, checks_run: Vec<String>) -> Report {
    let penalty: u32 = findings
        .iter()
        .map(|f| match f.severity {
            Severity::High => 30,
            Severity::Medium => 12,
            Severity::Low => 4,
            Severity::Info => 0,
        })
        .sum();
    let mut findings = findings;
    findings.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.id.cmp(&b.id)));
    Report {
        score: 100u32.saturating_sub(penalty),
        findings,
        checks_run,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tampering_added_and_removed_files() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.gguf"), b"model-a").unwrap();
        fs::write(d.path().join("b.gguf"), b"model-b").unwrap();
        fs::write(d.path().join("settings.json"), b"{}").unwrap(); // volatile
        let base = snapshot(d.path()).unwrap();
        assert_eq!(base.len(), 2);
        assert!(integrity_findings(&base, &snapshot(d.path()).unwrap()).is_empty());
        fs::write(d.path().join("a.gguf"), b"EVIL").unwrap();
        fs::remove_file(d.path().join("b.gguf")).unwrap();
        fs::write(d.path().join("dropper.so"), b"x").unwrap();
        fs::write(d.path().join("settings.json"), b"{\"x\":1}").unwrap();
        let f = integrity_findings(&base, &snapshot(d.path()).unwrap());
        let ids: Vec<_> = f.iter().map(|x| x.id.as_str()).collect();
        assert!(
            ids.contains(&"file_modified")
                && ids.contains(&"file_removed")
                && ids.contains(&"file_added")
        );
        assert_eq!(f.len(), 3, "volatile settings.json ignored");
    }

    #[test]
    fn parses_listening_sockets() {
        let t = "  sl  local_address rem_address   st tx_queue rx_queue\n\
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1 1 0\n\
   1: 0100007F:C350 0100007F:0050 01 00000000:00000000 00:00000000 00000000  1000        0 2 1 0\n\
   2: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 3 1 0\n";
        assert_eq!(listening_ports(t), vec![22, 8080]);
        assert!(listening_ports("").is_empty());
        assert_eq!(network_findings(&[8080]).len(), 1);
    }

    #[test]
    fn posture_and_score() {
        let h = HostSecurityFacts {
            screen_lock_set: Some(false),
            root_indicators: vec!["su binary".into()],
            adb_enabled: Some(true),
            security_patch: Some("2024-01-05".into()),
            ..Default::default()
        };
        let r = report(posture_findings(&h), vec!["posture".into()]);
        assert_eq!(r.findings[0].severity, Severity::High);
        assert_eq!(r.score, 100 - 30 - 30 - 12 - 4);
        let clean = report(posture_findings(&HostSecurityFacts::default()), vec![]);
        assert_eq!(clean.score, 100, "unknown facts are not penalized");
        assert!(report(vec![], vec![]).findings.is_empty());
    }
}
