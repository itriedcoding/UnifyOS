//! `unify-sysmond` — minimal sampler that publishes system state for the
//! desktop.
//!
//! It writes an atomic snapshot to `/run/unify/sysmon.json` plus a bounded
//! history ring in `/run/unify/sysmon-history.json`, which Unify System Monitor
//! graphs. It deliberately does not walk the process table: heavy sampling is
//! done on demand by `unify-ctl processes`.
//!
//! Cost at the default 1 s interval: one timer wakeup, three small procfs
//! reads, a handful of sysfs reads and two atomic writes. No threads, no
//! allocation-heavy loops, no polling of unrelated subsystems.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use unify_core::json::JsonWriter;
use unify_core::{hwmon, procfs};

const DEFAULT_DIR: &str = "/run/unify";
const HISTORY_SAMPLES: usize = 60;

fn main() {
    let mut interval_ms = 1000u64;
    let mut once = false;
    let mut directory = PathBuf::from(DEFAULT_DIR);

    let args: Vec<String> = env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--interval" => {
                if let Some(value) = args.get(index + 1).and_then(|value| value.parse().ok()) {
                    interval_ms = value;
                }
                index += 2;
            }
            "--dir" => {
                if let Some(value) = args.get(index + 1) {
                    directory = PathBuf::from(value);
                }
                index += 2;
            }
            "--once" => {
                once = true;
                index += 1;
            }
            "--help" => {
                println!(
                    "usage: unify-sysmond [--interval MILLISECONDS] [--dir DIRECTORY] [--once]"
                );
                return;
            }
            _ => index += 1,
        }
    }

    if let Err(error) = fs::create_dir_all(&directory) {
        eprintln!(
            "unify-sysmond: cannot create {}: {error}",
            directory.display()
        );
        std::process::exit(1);
    }

    let mut previous = procfs::cpu().unwrap_or_default();
    let mut history: Vec<String> = Vec::new();

    loop {
        if interval_ms > 0 {
            sleep(Duration::from_millis(interval_ms));
        }
        match snapshot(&previous) {
            Ok((document, current)) => {
                previous = current;
                if let Err(error) = write_atomic(&directory, "sysmon.json", &document) {
                    eprintln!("unify-sysmond: {error}");
                }
                history.push(document);
                while history.len() > HISTORY_SAMPLES {
                    history.remove(0);
                }
                let ring = format!("{{\"history\":[{}]}}", history.join(","));
                if let Err(error) = write_atomic(&directory, "sysmon-history.json", &ring) {
                    eprintln!("unify-sysmond: {error}");
                }
            }
            Err(error) => {
                eprintln!("unify-sysmond: {error}");
                if once {
                    std::process::exit(1);
                }
            }
        }
        if once {
            return;
        }
    }
}

fn snapshot(previous: &procfs::CpuSnapshot) -> Result<(String, procfs::CpuSnapshot), String> {
    let current = procfs::cpu().map_err(|error| format!("reading /proc/stat: {error}"))?;
    let memory = procfs::meminfo().map_err(|error| format!("reading /proc/meminfo: {error}"))?;
    let load = procfs::loadavg().unwrap_or_default();
    let uptime = procfs::uptime().unwrap_or(0.0);
    let sensors = hwmon::sensors();
    let batteries = hwmon::batteries();
    let root_space = unify_core::fs::space(Path::new("/")).ok();

    let mut w = JsonWriter::compact();
    w.begin_object(None);
    w.num_field("uptime_seconds", uptime);
    w.begin_object(Some("cpu"));
    w.num_field("usage_percent", current.usage_since(previous));
    w.uint_field("cores", current.core_count() as u64);
    w.begin_array(Some("per_core"));
    for usage in current.core_usage_since(previous) {
        w.num_value(usage);
    }
    w.end_array();
    w.end_object();
    w.begin_object(Some("memory"));
    w.uint_field("total_kb", memory.total_kb);
    w.uint_field("used_kb", memory.used_kb());
    w.num_field("used_percent", memory.used_percent());
    w.uint_field("swap_used_kb", memory.swap_used_kb());
    w.end_object();
    w.begin_object(Some("load"));
    w.num_field("one", load.one);
    w.num_field("five", load.five);
    w.num_field("fifteen", load.fifteen);
    w.end_object();
    w.begin_object(Some("storage"));
    match &root_space {
        Some(space) => {
            w.uint_field("total_bytes", space.total);
            w.uint_field("available_bytes", space.available);
            w.num_field("used_percent", space.used_percent());
        }
        None => w.null_field("total_bytes"),
    }
    w.end_object();
    w.begin_array(Some("sensors"));
    for sensor in &sensors {
        w.begin_object(None);
        w.str_field("chip", &sensor.chip);
        w.str_field("label", &sensor.label);
        w.str_field(
            "kind",
            match sensor.kind {
                hwmon::SensorKind::Temperature => "temperature",
                hwmon::SensorKind::Fan => "fan",
                hwmon::SensorKind::Voltage => "voltage",
            },
        );
        w.num_field("value", sensor.value);
        w.end_object();
    }
    w.end_array();
    w.begin_array(Some("batteries"));
    for battery in &batteries {
        w.begin_object(None);
        w.str_field("name", &battery.name);
        w.str_field("status", &battery.status);
        if let Some(capacity) = battery.capacity_percent {
            w.num_field("capacity_percent", capacity);
        }
        w.end_object();
    }
    w.end_array();
    w.end_object();
    Ok((w.finish(), current))
}

fn write_atomic(directory: &Path, name: &str, contents: &str) -> io::Result<()> {
    let target = directory.join(name);
    let temporary = directory.join(format!(".{name}.tmp"));
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, &target)
}
