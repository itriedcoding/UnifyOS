//! Parsers for Linux `/proc` and per-process metadata.
//!
//! All parsing functions take the raw text (or a directory path) as a
//! parameter so they can be unit-tested on fixture data without touching the
//! running system.

use std::fs;
use std::io;
use std::path::Path;

pub const PROC_STAT: &str = "/proc/stat";
pub const PROC_MEMINFO: &str = "/proc/meminfo";
pub const PROC_LOADAVG: &str = "/proc/loadavg";
pub const PROC_UPTIME: &str = "/proc/uptime";
pub const PROC_DISKSTATS: &str = "/proc/diskstats";

/// Kernel clock tick rate (`USER_HZ`); used to convert process CPU time.
pub const CLOCK_TICKS_PER_SECOND: u64 = 100;

/// Raw jiffy counters from one `cpu` line of `/proc/stat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuTimes {
    /// Sum of every counter, i.e. total jiffies observed.
    pub fn total(&self) -> u64 {
        self.user
            .saturating_add(self.nice)
            .saturating_add(self.system)
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
            .saturating_add(self.steal)
    }

    /// Jiffies the CPU spent not doing work.
    pub fn idle_total(&self) -> u64 {
        self.idle.saturating_add(self.iowait)
    }

    pub fn busy(&self) -> u64 {
        self.total().saturating_sub(self.idle_total())
    }

    /// Per-counter difference against an older sample.
    pub fn delta(&self, older: &CpuTimes) -> CpuTimes {
        CpuTimes {
            user: self.user.saturating_sub(older.user),
            nice: self.nice.saturating_sub(older.nice),
            system: self.system.saturating_sub(older.system),
            idle: self.idle.saturating_sub(older.idle),
            iowait: self.iowait.saturating_sub(older.iowait),
            irq: self.irq.saturating_sub(older.irq),
            softirq: self.softirq.saturating_sub(older.softirq),
            steal: self.steal.saturating_sub(older.steal),
        }
    }

    /// Busy percentage between two samples (0–100).
    pub fn usage_since(&self, older: &CpuTimes) -> f64 {
        let delta = self.delta(older);
        let total = delta.total();
        if total == 0 {
            0.0
        } else {
            (delta.busy() as f64) * 100.0 / (total as f64)
        }
    }
}

/// Aggregate plus per-core CPU counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuSnapshot {
    pub total: CpuTimes,
    pub per_core: Vec<CpuTimes>,
}

impl CpuSnapshot {
    pub fn core_count(&self) -> usize {
        self.per_core.len()
    }

    pub fn usage_since(&self, older: &CpuSnapshot) -> f64 {
        self.total.usage_since(&older.total)
    }

    pub fn core_usage_since(&self, older: &CpuSnapshot) -> Vec<f64> {
        self.per_core
            .iter()
            .zip(older.per_core.iter())
            .map(|(current, previous)| current.usage_since(previous))
            .collect()
    }
}

/// Parse the `cpu`/`cpuN` lines of `/proc/stat`.
pub fn parse_cpu(text: &str) -> CpuSnapshot {
    let mut snapshot = CpuSnapshot::default();
    for line in text.lines() {
        if !line.starts_with("cpu") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else {
            continue;
        };
        let values: Vec<u64> = fields.filter_map(|field| field.parse().ok()).collect();
        if values.len() < 4 {
            continue;
        }
        let times = CpuTimes {
            user: values[0],
            nice: values[1],
            system: values[2],
            idle: values[3],
            iowait: values.get(4).copied().unwrap_or(0),
            irq: values.get(5).copied().unwrap_or(0),
            softirq: values.get(6).copied().unwrap_or(0),
            steal: values.get(7).copied().unwrap_or(0),
        };
        if label == "cpu" {
            snapshot.total = times;
        } else {
            snapshot.per_core.push(times);
        }
    }
    snapshot
}

pub fn cpu() -> io::Result<CpuSnapshot> {
    Ok(parse_cpu(&fs::read_to_string(PROC_STAT)?))
}

/// Memory counters from `/proc/meminfo`, in kibibytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemInfo {
    pub total_kb: u64,
    pub free_kb: u64,
    pub available_kb: u64,
    pub buffers_kb: u64,
    pub cached_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub dirty_kb: u64,
}

impl MemInfo {
    /// Memory actually unavailable to new allocations.
    pub fn used_kb(&self) -> u64 {
        let available = if self.available_kb > 0 {
            self.available_kb
        } else {
            self.free_kb + self.buffers_kb + self.cached_kb
        };
        self.total_kb.saturating_sub(available)
    }

    pub fn used_percent(&self) -> f64 {
        if self.total_kb == 0 {
            0.0
        } else {
            (self.used_kb() as f64) * 100.0 / (self.total_kb as f64)
        }
    }

    pub fn swap_used_kb(&self) -> u64 {
        self.swap_total_kb.saturating_sub(self.swap_free_kb)
    }

    pub fn cached_total_kb(&self) -> u64 {
        self.cached_kb + self.buffers_kb
    }
}

pub fn parse_meminfo(text: &str) -> MemInfo {
    let mut info = MemInfo::default();
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value = rest
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        match key {
            "MemTotal" => info.total_kb = value,
            "MemFree" => info.free_kb = value,
            "MemAvailable" => info.available_kb = value,
            "Buffers" => info.buffers_kb = value,
            "Cached" => info.cached_kb = value,
            "SwapTotal" => info.swap_total_kb = value,
            "SwapFree" => info.swap_free_kb = value,
            "Dirty" => info.dirty_kb = value,
            _ => {}
        }
    }
    info
}

pub fn meminfo() -> io::Result<MemInfo> {
    Ok(parse_meminfo(&fs::read_to_string(PROC_MEMINFO)?))
}

/// Load average and runnable/total process counts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
    pub running: u32,
    pub total: u32,
}

pub fn parse_loadavg(text: &str) -> LoadAverage {
    let fields: Vec<&str> = text.split_whitespace().collect();
    let number = |index: usize| -> f64 {
        fields
            .get(index)
            .and_then(|value| value.replace(',', ".").parse().ok())
            .unwrap_or(0.0)
    };
    let (running, total) = fields
        .get(3)
        .and_then(|pair| pair.split_once('/'))
        .map(|(running, total)| {
            (
                running.parse::<u32>().unwrap_or(0),
                total.parse::<u32>().unwrap_or(0),
            )
        })
        .unwrap_or((0, 0));
    LoadAverage {
        one: number(0),
        five: number(1),
        fifteen: number(2),
        running,
        total,
    }
}

pub fn loadavg() -> io::Result<LoadAverage> {
    Ok(parse_loadavg(&fs::read_to_string(PROC_LOADAVG)?))
}

pub fn uptime() -> io::Result<f64> {
    let text = fs::read_to_string(PROC_UPTIME)?;
    Ok(text
        .split_whitespace()
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0))
}

/// Block-device I/O counters from `/proc/diskstats`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskStat {
    pub device: String,
    pub read_ios: u64,
    pub write_ios: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

pub fn parse_diskstats(text: &str) -> Vec<DiskStat> {
    let mut stats = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let number = |index: usize| -> u64 {
            fields
                .get(index)
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        };
        stats.push(DiskStat {
            device: fields[2].to_string(),
            read_ios: number(3),
            write_ios: number(7),
            read_bytes: number(5).saturating_mul(512),
            write_bytes: number(9).saturating_mul(512),
        });
    }
    stats
}

pub fn diskstats() -> io::Result<Vec<DiskStat>> {
    Ok(parse_diskstats(&fs::read_to_string(PROC_DISKSTATS)?))
}

/// Fields of `/proc/<pid>/stat` that UnifyOS needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub ppid: u32,
    pub utime: u64,
    pub stime: u64,
    pub threads: u64,
    pub rss_pages: u64,
}

impl ProcessStat {
    pub fn cpu_ticks(&self) -> u64 {
        self.utime.saturating_add(self.stime)
    }
}

/// Parse `/proc/<pid>/stat`. The command name may contain spaces and
/// parentheses, so the parser anchors on the last `)`.
pub fn parse_process_stat(pid: u32, text: &str) -> Option<ProcessStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    let comm = text[open + 1..close].to_string();
    let rest: Vec<&str> = text[close + 1..].split_whitespace().collect();
    let state = rest.first()?.chars().next()?;
    Some(ProcessStat {
        pid,
        comm,
        state,
        ppid: rest.get(1)?.parse().ok()?,
        utime: rest.get(11)?.parse().ok()?,
        stime: rest.get(12)?.parse().ok()?,
        threads: rest.get(17)?.parse().ok()?,
        rss_pages: rest.get(21)?.parse().ok()?,
    })
}

/// A process as reported to the desktop applications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub state: char,
    pub cpu_ticks: u64,
    pub threads: u64,
    pub rss_kb: u64,
    pub cmdline: String,
}

impl ProcessInfo {
    /// RSS in kibibytes, assuming the usual 4 KiB page size.
    pub fn rss_kb(&self) -> u64 {
        self.rss_kb
    }
}

fn cmdline_string(raw: &[u8]) -> String {
    let mut parts = Vec::new();
    for chunk in raw.split(|byte| *byte == 0) {
        if chunk.is_empty() {
            continue;
        }
        parts.push(String::from_utf8_lossy(chunk).to_string());
    }
    parts.join(" ")
}

/// Enumerate processes below `proc_root` (usually `/proc`).
pub fn processes(proc_root: &Path) -> io::Result<Vec<ProcessInfo>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(proc_root)? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat_text) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(stat) = parse_process_stat(pid, &stat_text) else {
            continue;
        };
        let cmdline = fs::read(entry.path().join("cmdline"))
            .map(|raw| cmdline_string(&raw))
            .unwrap_or_default();
        let cpu_ticks = stat.cpu_ticks();
        result.push(ProcessInfo {
            pid,
            ppid: stat.ppid,
            name: stat.comm,
            state: stat.state,
            cpu_ticks,
            threads: stat.threads,
            rss_kb: stat.rss_pages.saturating_mul(4),
            cmdline,
        });
    }
    result.sort_by_key(|process| process.pid);
    Ok(result)
}

/// Read one process from `/proc/<pid>`.
pub fn process(pid: u32) -> io::Result<Option<ProcessInfo>> {
    let base = format!("/proc/{pid}");
    let Ok(stat_text) = fs::read_to_string(format!("{base}/stat")) else {
        return Ok(None);
    };
    let Some(stat) = parse_process_stat(pid, &stat_text) else {
        return Ok(None);
    };
    let cmdline = fs::read(format!("{base}/cmdline"))
        .map(|raw| cmdline_string(&raw))
        .unwrap_or_default();
    let cpu_ticks = stat.cpu_ticks();
    Ok(Some(ProcessInfo {
        pid,
        ppid: stat.ppid,
        name: stat.comm,
        state: stat.state,
        cpu_ticks,
        threads: stat.threads,
        rss_kb: stat.rss_pages.saturating_mul(4),
        cmdline,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPU_FIXTURE: &str = "cpu  100 20 30 400 10 0 5 0 0 0\n\
cpu0 50 10 15 200 5 0 2 0 0 0\n\
cpu1 50 10 15 200 5 0 3 0 0 0\n\
intr 1234 0 0\n\
ctxt 5678\n";

    #[test]
    fn parses_total_and_per_core() {
        let snapshot = parse_cpu(CPU_FIXTURE);
        assert_eq!(snapshot.core_count(), 2);
        assert_eq!(snapshot.total.user, 100);
        assert_eq!(snapshot.total.iowait, 10);
        assert_eq!(snapshot.total.total(), 565);
        assert_eq!(snapshot.total.idle_total(), 410);
    }

    #[test]
    fn computes_usage_between_samples() {
        let older = parse_cpu(CPU_FIXTURE);
        let newer = parse_cpu(
            "cpu  120 20 30 420 10 0 5 0 0 0\n\
cpu0 60 10 15 210 5 0 2 0 0 0\n\
cpu1 60 10 15 210 5 0 3 0 0 0\n",
        );
        // 20 user + 20 idle jiffies elapsed, all 20 of the user jiffies busy.
        let usage = newer.usage_since(&older);
        assert!((usage - 50.0).abs() < 1e-9, "usage was {usage}");
        let cores = newer.core_usage_since(&older);
        assert_eq!(cores.len(), 2);
    }

    #[test]
    fn parses_meminfo() {
        let info = parse_meminfo(
            "MemTotal:        3928440 kB\n\
MemFree:          291000 kB\n\
MemAvailable:    2012340 kB\n\
Buffers:           82000 kB\n\
Cached:          1500000 kB\n\
SwapTotal:       1048572 kB\n\
SwapFree:        1048000 kB\n",
        );
        assert_eq!(info.total_kb, 3_928_440);
        assert_eq!(info.used_kb(), 3_928_440 - 2_012_340);
        assert_eq!(info.swap_used_kb(), 572);
        assert!((info.used_percent() - 48.78).abs() < 0.05);
    }

    #[test]
    fn parses_loadavg() {
        let load = parse_loadavg("0.52 0.58 0.59 2/812 3201");
        assert!((load.one - 0.52).abs() < 1e-9);
        assert_eq!(load.running, 2);
        assert_eq!(load.total, 812);
    }

    #[test]
    fn parses_diskstats_sectors_into_bytes() {
        let stats = parse_diskstats(
            "   8       0 sda 1000 0 20000 500 2000 0 40000 900 0 0 0\n\
   8       1 sda1 10 0 200 5 20 0 400 9 0 0 0\n",
        );
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].device, "sda");
        assert_eq!(stats[0].read_bytes, 20_000 * 512);
        assert_eq!(stats[0].write_bytes, 40_000 * 512);
    }

    #[test]
    fn parses_process_stat_with_spaces_in_comm() {
        let text = "4242 (Web Content (x)) S 1 4242 4242 0 -1 4194304 1000 0 0 0 \
50 70 0 0 20 0 8 0 12345 1000000 2500 18446744073709551615 0 0 0";
        let stat = parse_process_stat(4242, text).expect("parse");
        assert_eq!(stat.comm, "Web Content (x)");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.ppid, 1);
        assert_eq!(stat.cpu_ticks(), 120);
        assert_eq!(stat.threads, 8);
        assert_eq!(stat.rss_pages, 2500);
    }

    #[test]
    fn rejects_broken_process_stat() {
        assert!(parse_process_stat(1, "garbage").is_none());
    }

    #[test]
    fn splits_null_separated_cmdline() {
        assert_eq!(cmdline_string(b"unify-ctl\0sysinfo\0"), "unify-ctl sysinfo");
        assert_eq!(cmdline_string(b""), "");
    }
}
