//! Device capability collection.
//!
//! Collects what can be read reliably from Rust on Linux/Android (procfs,
//! statvfs). Information that only the Android framework knows reliably
//! (thermal status, GPU/NPU availability for a given engine, low-RAM flag) is
//! supplied by the host through [`HostHints`] — the profiler never guesses it.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Coarse thermal state as reported by Android's `PowerManager`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalState {
    None,
    Light,
    Moderate,
    Severe,
    Critical,
}

/// Facts provided by the host platform (Kotlin side). All optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostHints {
    pub thermal: Option<ThermalState>,
    /// Only set when the *inference engine* has confirmed it can use the GPU.
    pub gpu_offload_supported: Option<bool>,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// CPU instruction-set features that speed up inference (e.g. neon,
    /// dotprod, i8mm, sve, avx2, avx512f), parsed from /proc/cpuinfo.
    #[serde(default)]
    pub cpu_features: Vec<String>,
    /// Core clock classes (max kHz) from sysfs, to tell big from little cores.
    #[serde(default)]
    pub core_max_khz: Vec<u32>,
    /// Devices with >= 12 GiB RAM, 8+ cores and fast features: "flagship".
    #[serde(default)]
    pub tier: String,
    /// e.g. "aarch64", "x86_64".
    pub arch: String,
    pub cpu_cores: u32,
    pub total_ram_bytes: u64,
    /// Memory the kernel estimates is available without swapping.
    pub available_ram_bytes: u64,
    /// Free bytes on the volume where models are stored (for unprivileged user).
    pub free_storage_bytes: u64,
    pub hints: HostHints,
}

impl DeviceProfile {
    /// Rough RAM class in GiB (4, 6, 8, 12, 16, ...). An *input hint* only —
    /// selection decisions use the real byte counts.
    pub fn ram_class_gb(&self) -> u32 {
        let gib = self.total_ram_bytes as f64 / (1u64 << 30) as f64;
        // Devices advertise e.g. "8 GB" but report ~7.4 GiB usable.
        const CLASSES: [u32; 7] = [2, 3, 4, 6, 8, 12, 16];
        CLASSES
            .iter()
            .copied()
            .find(|&c| gib <= c as f64 * 1.0)
            .unwrap_or_else(|| gib.ceil() as u32)
    }
}

/// Parsed subset of `/proc/meminfo` (values in bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemInfo {
    pub total: u64,
    pub available: u64,
}

/// Parses `/proc/meminfo` content. Falls back to `MemFree + Cached` for very
/// old kernels without `MemAvailable`.
pub fn parse_meminfo(text: &str) -> AraResult<MemInfo> {
    let mut total = None;
    let mut available = None;
    let mut free = None;
    let mut cached = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(key), Some(val)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(kb) = val.parse::<u64>() else { continue };
        let bytes = kb.saturating_mul(1024);
        match key {
            "MemTotal:" => total = Some(bytes),
            "MemAvailable:" => available = Some(bytes),
            "MemFree:" => free = Some(bytes),
            "Cached:" => cached = Some(bytes),
            _ => {}
        }
    }
    let total = total.ok_or_else(|| AraError::Unsupported("meminfo without MemTotal".into()))?;
    let available = available
        .or_else(|| Some(free? + cached.unwrap_or(0)))
        .ok_or_else(|| AraError::Unsupported("meminfo without MemAvailable/MemFree".into()))?;
    Ok(MemInfo {
        total,
        available: available.min(total),
    })
}

const USEFUL_FEATURES: &[&str] = &[
    "neon", "asimd", "asimddp", "dotprod", "i8mm", "bf16", "sve", "sve2", "fphp", "avx", "avx2",
    "avx512f", "fma", "f16c",
];

/// Extracts inference-relevant CPU features from /proc/cpuinfo text.
pub fn parse_cpu_features(text: &str) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    for line in text.lines() {
        let l = line.to_ascii_lowercase();
        if l.starts_with("flags") || l.starts_with("features") {
            if let Some((_, v)) = l.split_once(':') {
                for f in v.split_whitespace() {
                    if USEFUL_FEATURES.contains(&f) {
                        found.insert(match f {
                            "asimd" => "neon".to_string(),
                            "asimddp" => "dotprod".to_string(),
                            other => other.to_string(),
                        });
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}

/// Classifies the device from measurable facts (not marketing names).
pub fn classify_tier(total_ram: u64, cores: u32, features: &[String]) -> &'static str {
    let gib = total_ram as f64 / (1u64 << 30) as f64;
    let fast = features
        .iter()
        .any(|f| ["i8mm", "sve2", "avx512f", "dotprod", "avx2"].contains(&f.as_str()));
    if gib >= 11.0 && cores >= 8 && fast {
        "flagship"
    } else if gib >= 5.5 && cores >= 6 {
        "mid"
    } else {
        "entry"
    }
}

fn read_core_khz() -> Vec<u32> {
    let mut v = Vec::new();
    for i in 0..64 {
        let p = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq");
        match std::fs::read_to_string(p) {
            Ok(s) => v.push(s.trim().parse().unwrap_or(0)),
            Err(_) => {
                if !std::path::Path::new(&format!("/sys/devices/system/cpu/cpu{i}")).exists() {
                    break;
                }
            }
        }
    }
    v
}

/// Resident memory of the current process, from `/proc/self/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessMemory {
    pub rss_bytes: u64,
    /// High-water mark of RSS since process start.
    pub peak_rss_bytes: u64,
}

pub fn parse_proc_status(text: &str) -> AraResult<ProcessMemory> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    };
    match (field("VmRSS:"), field("VmHWM:")) {
        (Some(rss), Some(peak)) => Ok(ProcessMemory {
            rss_bytes: rss,
            peak_rss_bytes: peak.max(rss),
        }),
        _ => Err(AraError::Unsupported("status without VmRSS/VmHWM".into())),
    }
}

pub fn process_memory() -> AraResult<ProcessMemory> {
    parse_proc_status(&std::fs::read_to_string("/proc/self/status")?)
}

/// Free space for an unprivileged process on the filesystem holding `path`.
#[cfg(unix)]
pub fn free_storage_bytes(path: &Path) -> AraResult<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AraError::InvalidInput("path contains NUL".into()))?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid NUL-terminated string and `st` is a valid out-ptr.
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    #[allow(clippy::unnecessary_cast)]
    Ok((st.f_bavail as u64).saturating_mul(st.f_frsize as u64))
}

#[cfg(not(unix))]
pub fn free_storage_bytes(_path: &Path) -> AraResult<u64> {
    Err(AraError::Unsupported(
        "storage query only implemented on unix".into(),
    ))
}

/// Collects a profile for the current device. `model_dir` is where models
/// would be stored (its filesystem is measured).
pub fn collect(model_dir: &Path, hints: HostHints) -> AraResult<DeviceProfile> {
    let mem = parse_meminfo(&std::fs::read_to_string("/proc/meminfo")?)?;
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let cpu_features =
        parse_cpu_features(&std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default());
    let tier = classify_tier(mem.total, cpu_cores, &cpu_features).to_string();
    Ok(DeviceProfile {
        cpu_features,
        core_max_khz: read_core_khz(),
        tier,
        arch: std::env::consts::ARCH.to_string(),
        cpu_cores,
        total_ram_bytes: mem.total,
        available_ram_bytes: mem.available,
        free_storage_bytes: free_storage_bytes(model_dir)?,
        hints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANDROID_MEMINFO: &str = "MemTotal:        7645328 kB\n\
MemFree:          312400 kB\n\
MemAvailable:    3120400 kB\n\
Buffers:            2000 kB\n\
Cached:          2500000 kB\n";

    #[test]
    fn parses_meminfo() {
        let m = parse_meminfo(ANDROID_MEMINFO).unwrap();
        assert_eq!(m.total, 7645328 * 1024);
        assert_eq!(m.available, 3120400 * 1024);
    }

    #[test]
    fn meminfo_fallback_without_memavailable() {
        let m = parse_meminfo("MemTotal: 1000 kB\nMemFree: 100 kB\nCached: 50 kB\n").unwrap();
        assert_eq!(m.available, 150 * 1024);
    }

    #[test]
    fn meminfo_rejects_garbage() {
        assert!(parse_meminfo("").is_err());
        assert!(parse_meminfo("MemTotal: abc kB").is_err());
    }

    fn profile_with_total(total: u64) -> DeviceProfile {
        DeviceProfile {
            cpu_features: vec![],
            core_max_khz: vec![],
            tier: String::new(),
            arch: "aarch64".into(),
            cpu_cores: 8,
            total_ram_bytes: total,
            available_ram_bytes: total / 2,
            free_storage_bytes: 0,
            hints: HostHints::default(),
        }
    }

    #[test]
    fn ram_class_rounds_up_to_marketing_size() {
        assert_eq!(profile_with_total(7645328 * 1024).ram_class_gb(), 8);
        assert_eq!(profile_with_total(3_800_000 * 1024).ram_class_gb(), 4);
        assert_eq!(profile_with_total(5_600_000 * 1024).ram_class_gb(), 6);
        assert_eq!(profile_with_total(15_500_000 * 1024).ram_class_gb(), 16);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn collects_real_profile_on_linux() {
        let dir = std::env::temp_dir();
        let p = collect(&dir, HostHints::default()).unwrap();
        assert!(p.total_ram_bytes > 0);
        assert!(p.available_ram_bytes <= p.total_ram_bytes);
        assert!(p.cpu_cores >= 1);
        assert!(p.free_storage_bytes > 0);
    }

    #[test]
    fn cpu_features_and_tier() {
        let arm = "Features\t: fp asimd evtstrm aes asimddp i8mm bf16\nCPU part\t: 0xd4e";
        let f = parse_cpu_features(arm);
        assert_eq!(f, ["bf16", "dotprod", "i8mm", "neon"]);
        assert_eq!(classify_tier(12 << 30, 8, &f), "flagship");
        assert_eq!(classify_tier(8 << 30, 8, &f), "mid");
        assert_eq!(classify_tier(3 << 30, 4, &[]), "entry");
        assert!(parse_cpu_features("flags : sse2 avx2 fma").contains(&"avx2".to_string()));
    }

    #[test]
    fn parses_process_status() {
        let m = parse_proc_status("Name:\tara\nVmHWM:\t  2048 kB\nVmRSS:\t  1024 kB\n").unwrap();
        assert_eq!(m.rss_bytes, 1024 * 1024);
        assert_eq!(m.peak_rss_bytes, 2048 * 1024);
        assert!(parse_proc_status("Name: x").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_own_process_memory() {
        let m = process_memory().unwrap();
        assert!(m.rss_bytes > 0 && m.peak_rss_bytes >= m.rss_bytes);
    }

    #[test]
    fn storage_query_fails_cleanly_for_missing_path() {
        assert!(free_storage_bytes(Path::new("/definitely/not/here")).is_err());
    }
}
