//! `unify-ctl` — UnifyOS system control CLI.
//!
//! Single backend for Unify Settings, Unify Control Center, Unify System
//! Monitor and the Unify panel. Every command talks to the real system:
//! procfs/sysfs, systemd-logind, NetworkManager (`nmcli`), PipeWire (`wpctl`)
//! and BlueZ (`bluetoothctl`). `--json` output is the stable machine interface
//! documented in `documentation/architecture.md`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use unify_core::exec;
use unify_core::fs as ufs;
use unify_core::hwmon;
use unify_core::json::JsonWriter;
use unify_core::procfs;

type CmdResult = Result<(), String>;

const BACKLIGHT_ROOT: &str = "/sys/class/backlight";

struct Args {
    command: String,
    rest: Vec<String>,
    json: bool,
}

impl Args {
    fn parse() -> Self {
        let mut json = false;
        let mut words = Vec::new();
        for arg in env::args().skip(1) {
            match arg.as_str() {
                "--json" | "-j" => json = true,
                other => words.push(other.to_string()),
            }
        }
        let command = words.first().cloned().unwrap_or_default();
        let rest = if words.is_empty() {
            Vec::new()
        } else {
            words[1..].to_vec()
        };
        Self {
            command,
            rest,
            json,
        }
    }

    fn word(&self, index: usize) -> Option<&str> {
        self.rest.get(index).map(String::as_str)
    }

    fn flag_value(&self, flag: &str) -> Option<&str> {
        self.rest
            .iter()
            .position(|word| word == flag)
            .and_then(|index| self.rest.get(index + 1))
            .map(String::as_str)
    }
}

fn main() {
    let args = Args::parse();
    let outcome = match args.command.as_str() {
        "sysinfo" => cmd_sysinfo(args.json),
        "cpu" => cmd_cpu(args.json),
        "mem" => cmd_mem(args.json),
        "disk" => cmd_disk(args.json),
        "sensors" => cmd_sensors(args.json),
        "battery" => cmd_battery(args.json),
        "power" => cmd_power(&args),
        "brightness" => cmd_brightness(&args),
        "volume" => cmd_volume(&args),
        "network" => cmd_network(&args),
        "bluetooth" => cmd_bluetooth(&args),
        "display" => cmd_display(args.json),
        "processes" => cmd_processes(&args),
        "version" | "--version" => {
            println!("unify-ctl {} ({})", unify_core::VERSION, env::consts::ARCH);
            Ok(())
        }
        "help" | "" => {
            usage();
            Ok(())
        }
        other => Err(format!(
            "unknown command `{other}`; run `unify-ctl help` for the command list"
        )),
    };
    if let Err(message) = outcome {
        eprintln!("unify-ctl: {message}");
        exit(1);
    }
}

fn usage() {
    println!(
        "unify-ctl {version} — UnifyOS system control\n\
\n\
Usage: unify-ctl <command> [args] [--json]\n\
\n\
Information\n\
  sysinfo                     full system snapshot\n\
  cpu                         CPU utilisation (200 ms sample)\n\
  mem                         memory and swap usage\n\
  disk                        block devices and filesystem usage\n\
  sensors                     temperatures, fans and voltages\n\
  battery                     battery state and runtime estimate\n\
  processes [--sort cpu|mem] [--limit N]\n\
\n\
Hardware control\n\
  brightness [get|set N|up|down]\n\
  volume [get|set N|mute|unmute|toggle]\n\
  display                     connected outputs and modes\n\
\n\
Networking\n\
  network status|wifi-list|wifi-connect <ssid> [password]|wifi-disconnect|enable|disable\n\
  bluetooth status|devices|scan|connect <mac>|power on|off\n\
\n\
Power\n\
  power status|suspend|reboot|poweroff|lock\n",
        version = unify_core::VERSION
    );
}

fn sample_cpu(window_ms: u64) -> Result<(f64, Vec<f64>), String> {
    let first = procfs::cpu().map_err(|error| format!("reading /proc/stat: {error}"))?;
    sleep(Duration::from_millis(window_ms));
    let second = procfs::cpu().map_err(|error| format!("reading /proc/stat: {error}"))?;
    Ok((second.usage_since(&first), second.core_usage_since(&first)))
}

fn storage_mounts() -> Vec<(String, ufs::Space)> {
    let mut result = Vec::new();
    let Ok(mounts) = ufs::mounts() else {
        return result;
    };
    for mount in mounts {
        if !mount.source.starts_with("/dev/") {
            continue;
        }
        if result.iter().any(|(target, _)| *target == mount.target) {
            continue;
        }
        if let Ok(space) = ufs::space(Path::new(&mount.target)) {
            result.push((mount.target.clone(), space));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn cmd_sysinfo(json: bool) -> CmdResult {
    let (usage, per_core) = sample_cpu(200)?;
    let mem = procfs::meminfo().map_err(|error| format!("reading /proc/meminfo: {error}"))?;
    let load = procfs::loadavg().unwrap_or_default();
    let uptime_seconds = procfs::uptime().unwrap_or(0.0);
    let sensors = hwmon::sensors();
    let batteries = hwmon::batteries();
    let hostname = ufs::read_trimmed(Path::new("/proc/sys/kernel/hostname")).unwrap_or_default();
    let kernel = ufs::read_trimmed(Path::new("/proc/sys/kernel/osrelease")).unwrap_or_default();
    let process_count = procfs::processes(Path::new("/proc"))
        .map(|list| list.len())
        .unwrap_or(0);
    let storage = storage_mounts();

    if !json {
        println!("UnifyOS {} — {} ({})", unify_core::VERSION, hostname, kernel);
        println!("uptime      {:.1} min", uptime_seconds / 60.0);
        println!("cpu         {:.1}% over {} cores", usage, per_core.len());
        println!(
            "memory      {} MiB used of {} MiB ({:.1}%)",
            mem.used_kb() / 1024,
            mem.total_kb / 1024,
            mem.used_percent()
        );
        println!(
            "swap        {} MiB used of {} MiB",
            mem.swap_used_kb() / 1024,
            mem.swap_total_kb / 1024
        );
        println!(
            "load        {:.2} {:.2} {:.2} ({} runnable of {})",
            load.one, load.five, load.fifteen, load.running, load.total
        );
        println!("processes   {process_count}");
        for (target, space) in &storage {
            println!(
                "storage     {target:<12} {:.2} GiB free of {:.2} GiB ({:.0}% used)",
                space.available as f64 / 1e9,
                space.total as f64 / 1e9,
                space.used_percent()
            );
        }
        for sensor in &sensors {
            println!(
                "sensor      {:<12} {:<18} {:>7.1}{}",
                sensor.chip,
                sensor.label,
                sensor.value,
                sensor.kind.unit()
            );
        }
        for battery in &batteries {
            println!(
                "battery     {} {} {:.0}%{}",
                battery.name,
                battery.status,
                battery.capacity_percent.unwrap_or(0.0),
                battery
                    .seconds_remaining()
                    .map(|seconds| format!(" ({:.1} h remaining)", seconds as f64 / 3600.0))
                    .unwrap_or_default()
            );
        }
        return Ok(());
    }

    let mut w = JsonWriter::compact();
    w.begin_object(None);
    w.str_field("unify_version", unify_core::VERSION);
    w.str_field("hostname", &hostname);
    w.str_field("kernel", &kernel);
    w.str_field("arch", env::consts::ARCH);
    w.num_field("uptime_seconds", uptime_seconds);
    w.uint_field("processes", process_count as u64);
    w.begin_object(Some("cpu"));
    w.num_field("usage_percent", usage);
    w.uint_field("cores", per_core.len() as u64);
    w.begin_array(Some("per_core"));
    for core in &per_core {
        w.num_value(*core);
    }
    w.end_array();
    w.begin_object(Some("load"));
    w.num_field("one", load.one);
    w.num_field("five", load.five);
    w.num_field("fifteen", load.fifteen);
    w.uint_field("running", load.running as u64);
    w.uint_field("total", load.total as u64);
    w.end_object();
    w.end_object();
    w.begin_object(Some("memory"));
    w.uint_field("total_kb", mem.total_kb);
    w.uint_field("used_kb", mem.used_kb());
    w.uint_field("available_kb", mem.available_kb);
    w.uint_field("cached_kb", mem.cached_total_kb());
    w.uint_field("swap_total_kb", mem.swap_total_kb);
    w.uint_field("swap_used_kb", mem.swap_used_kb());
    w.num_field("used_percent", mem.used_percent());
    w.end_object();
    w.begin_array(Some("storage"));
    for (target, space) in &storage {
        w.begin_object(None);
        w.str_field("mount", target);
        w.uint_field("total_bytes", space.total);
        w.uint_field("available_bytes", space.available);
        w.num_field("used_percent", space.used_percent());
        w.end_object();
    }
    w.end_array();
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
        w.str_field("unit", sensor.kind.unit());
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
        } else {
            w.null_field("capacity_percent");
        }
        if let Some(energy) = battery.energy_now_wh {
            w.num_field("energy_now_wh", energy);
        } else {
            w.null_field("energy_now_wh");
        }
        if let Some(power) = battery.power_now_w {
            w.num_field("power_now_w", power);
        } else {
            w.null_field("power_now_w");
        }
        if let Some(seconds) = battery.seconds_remaining() {
            w.uint_field("seconds_remaining", seconds);
        } else {
            w.null_field("seconds_remaining");
        }
        w.end_object();
    }
    w.end_array();
    w.end_object();
    println!("{}", w.finish());
    Ok(())
}

fn cmd_cpu(json: bool) -> CmdResult {
    let (usage, per_core) = sample_cpu(300)?;
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.num_field("usage_percent", usage);
        w.uint_field("cores", per_core.len() as u64);
        w.begin_array(Some("per_core"));
        for core in &per_core {
            w.num_value(*core);
        }
        w.end_array();
        w.end_object();
        println!("{}", w.finish());
    } else {
        println!("cpu total {usage:.1}%");
        for (index, core) in per_core.iter().enumerate() {
            println!("cpu {index} {core:.1}%");
        }
    }
    Ok(())
}

fn cmd_mem(json: bool) -> CmdResult {
    let mem = procfs::meminfo().map_err(|error| format!("reading /proc/meminfo: {error}"))?;
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.uint_field("total_kb", mem.total_kb);
        w.uint_field("used_kb", mem.used_kb());
        w.uint_field("available_kb", mem.available_kb);
        w.uint_field("cached_kb", mem.cached_total_kb());
        w.uint_field("swap_total_kb", mem.swap_total_kb);
        w.uint_field("swap_used_kb", mem.swap_used_kb());
        w.num_field("used_percent", mem.used_percent());
        w.end_object();
        println!("{}", w.finish());
    } else {
        println!(
            "memory {} MiB used of {} MiB ({:.1}%)",
            mem.used_kb() / 1024,
            mem.total_kb / 1024,
            mem.used_percent()
        );
        println!(
            "swap   {} MiB used of {} MiB",
            mem.swap_used_kb() / 1024,
            mem.swap_total_kb / 1024
        );
    }
    Ok(())
}

fn cmd_disk(json: bool) -> CmdResult {
    let devices = ufs::block_devices().map_err(|error| format!("reading /sys/block: {error}"))?;
    let storage = storage_mounts();
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.begin_array(Some("devices"));
        for device in &devices {
            w.begin_object(None);
            w.str_field("name", &device.name);
            w.uint_field("size_bytes", device.size_bytes);
            w.bool_field("removable", device.removable);
            w.bool_field("rotational", device.rotational);
            w.str_field("model", &device.model);
            w.begin_array(Some("partitions"));
            for partition in &device.partitions {
                w.str_value(partition);
            }
            w.end_array();
            w.end_object();
        }
        w.end_array();
        w.begin_array(Some("filesystems"));
        for (target, space) in &storage {
            w.begin_object(None);
            w.str_field("mount", target);
            w.uint_field("total_bytes", space.total);
            w.uint_field("available_bytes", space.available);
            w.num_field("used_percent", space.used_percent());
            w.end_object();
        }
        w.end_array();
        w.end_object();
        println!("{}", w.finish());
    } else {
        for device in &devices {
            println!(
                "{:<10} {:>8.2} GiB {}{} {}",
                device.name,
                device.size_bytes as f64 / 1e9,
                if device.removable { "removable " } else { "" },
                if device.rotational { "hdd" } else { "ssd" },
                device.model
            );
            for partition in &device.partitions {
                println!("  └─ {partition}");
            }
        }
        for (target, space) in &storage {
            println!(
                "{:<14} {:.2} GiB free of {:.2} GiB ({:.0}% used)",
                target,
                space.available as f64 / 1e9,
                space.total as f64 / 1e9,
                space.used_percent()
            );
        }
    }
    Ok(())
}

fn cmd_sensors(json: bool) -> CmdResult {
    let sensors = hwmon::sensors();
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
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
            w.str_field("unit", sensor.kind.unit());
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else {
        for sensor in &sensors {
            println!(
                "{:<12} {:<20} {:>8.1}{}",
                sensor.chip,
                sensor.label,
                sensor.value,
                sensor.kind.unit()
            );
        }
    }
    Ok(())
}

fn cmd_battery(json: bool) -> CmdResult {
    let batteries = hwmon::batteries();
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for battery in &batteries {
            w.begin_object(None);
            w.str_field("name", &battery.name);
            w.bool_field("present", battery.present);
            w.str_field("status", &battery.status);
            if let Some(capacity) = battery.capacity_percent {
                w.num_field("capacity_percent", capacity);
            }
            if let Some(seconds) = battery.seconds_remaining() {
                w.uint_field("seconds_remaining", seconds);
            }
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else if batteries.is_empty() {
        println!("no battery present (desktop or virtual machine)");
    } else {
        for battery in &batteries {
            println!(
                "{}: {} {:.0}%{}",
                battery.name,
                battery.status,
                battery.capacity_percent.unwrap_or(0.0),
                battery
                    .seconds_remaining()
                    .map(|seconds| format!(", {:.1} h remaining", seconds as f64 / 3600.0))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn cmd_power(args: &Args) -> CmdResult {
    let action = args.word(0).unwrap_or("status");
    match action {
        "status" => {
            let batteries = hwmon::batteries();
            let can_suspend = ufs::read_trimmed(Path::new("/sys/power/state"))
                .map(|states| states.split_whitespace().any(|state| state == "mem"))
                .unwrap_or(false);
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_object(None);
                w.bool_field("can_suspend", can_suspend);
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
                println!("{}", w.finish());
            } else {
                println!("suspend supported: {}", if can_suspend { "yes" } else { "no" });
                for battery in &batteries {
                    println!(
                        "{}: {} {:.0}%",
                        battery.name,
                        battery.status,
                        battery.capacity_percent.unwrap_or(0.0)
                    );
                }
            }
            Ok(())
        }
        "suspend" => systemctl_action("suspend"),
        "reboot" => systemctl_action("reboot"),
        "poweroff" => systemctl_action("poweroff"),
        "lock" => run_tool("loginctl", &["lock-sessions"]),
        other => Err(format!(
            "unknown power action `{other}` (status, suspend, reboot, poweroff, lock)"
        )),
    }
}

fn systemctl_action(action: &str) -> CmdResult {
    if !exec::have("systemctl") {
        return Err("systemctl is not available; cannot control power state".into());
    }
    let output = exec::run("systemctl", &[action]).map_err(|error| error.to_string())?;
    if !output.success() {
        return Err(output.failure_message());
    }
    println!("{action} requested");
    Ok(())
}

fn run_tool(program: &str, args: &[&str]) -> CmdResult {
    if !exec::have(program) {
        return Err(format!("{program} is not installed"));
    }
    let output = exec::run(program, args).map_err(|error| error.to_string())?;
    if !output.success() {
        return Err(output.failure_message());
    }
    if !output.stdout_trimmed().is_empty() {
        println!("{}", output.stdout_trimmed());
    }
    Ok(())
}

fn cmd_brightness(args: &Args) -> CmdResult {
    let backlights = hwmon::read_backlights(Path::new(BACKLIGHT_ROOT));
    let Some(primary) = backlights.first() else {
        return Err("no backlight device found (external display or unsupported GPU)".into());
    };
    let action = args.word(0).unwrap_or("get");
    match action {
        "get" => {
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_object(None);
                w.str_field("device", &primary.device);
                w.uint_field("brightness", primary.brightness);
                w.uint_field("max_brightness", primary.max_brightness);
                w.num_field("percent", primary.percent());
                w.end_object();
                println!("{}", w.finish());
            } else {
                println!("{:.0}%", primary.percent());
            }
            Ok(())
        }
        "set" | "up" | "down" => {
            let current = primary.percent();
            let target = match action {
                "set" => args
                    .word(1)
                    .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
                    .ok_or_else(|| "usage: unify-ctl brightness set <0-100>".to_string())?,
                "up" => current + 5.0,
                _ => current - 5.0,
            };
            let target = target.clamp(1.0, 100.0);
            let raw = ((target / 100.0) * primary.max_brightness as f64).round() as u64;
            let path = PathBuf::from(BACKLIGHT_ROOT)
                .join(&primary.device)
                .join("brightness");
            fs::write(&path, format!("{raw}\n")).map_err(|error| {
                format!(
                    "cannot write {}: {error} (add the user to group `video`; \
                     UnifyOS ships /usr/lib/udev/rules.d/90-unify-backlight.rules)",
                    path.display()
                )
            })?;
            println!("{target:.0}%");
            Ok(())
        }
        other => Err(format!("unknown brightness action `{other}`")),
    }
}

fn audio_available() -> bool {
    exec::have("wpctl") || exec::have("pactl")
}

fn volume_state() -> Result<(f64, bool), String> {
    if exec::have("wpctl") {
        let output =
            exec::run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]).map_err(|e| e.to_string())?;
        if !output.success() {
            return Err(output.failure_message());
        }
        let text = output.stdout_trimmed();
        let level = text
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<f64>().ok())
            .ok_or_else(|| format!("unexpected wpctl output: {text}"))?;
        return Ok((level * 100.0, text.contains("MUTED")));
    }
    if exec::have("pactl") {
        let output = exec::run("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])
            .map_err(|error| error.to_string())?;
        if !output.success() {
            return Err(output.failure_message());
        }
        let percent = output
            .stdout
            .split('%')
            .next()
            .and_then(|prefix| prefix.split_whitespace().last())
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0);
        let muted = exec::run("pactl", &["get-sink-mute", "@DEFAULT_SINK@"])
            .map(|output| output.stdout.contains("yes"))
            .unwrap_or(false);
        return Ok((percent, muted));
    }
    Err("no audio backend found (install PipeWire or pulseaudio-utils)".into())
}

fn cmd_volume(args: &Args) -> CmdResult {
    if !audio_available() {
        return Err("no audio backend found (install PipeWire or pulseaudio-utils)".into());
    }
    let action = args.word(0).unwrap_or("get");
    match action {
        "get" => {
            let (percent, muted) = volume_state()?;
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_object(None);
                w.num_field("percent", percent);
                w.bool_field("muted", muted);
                w.end_object();
                println!("{}", w.finish());
            } else {
                println!("{percent:.0}%{}", if muted { " (muted)" } else { "" });
            }
            Ok(())
        }
        "set" => {
            let percent = args
                .word(1)
                .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
                .ok_or_else(|| "usage: unify-ctl volume set <0-150>".to_string())?
                .clamp(0.0, 150.0);
            let level = format!("{:.2}", percent / 100.0);
            if exec::have("wpctl") {
                run_tool(
                    "wpctl",
                    &["set-volume", "@DEFAULT_AUDIO_SINK@", level.as_str()],
                )?;
            } else {
                let level = format!("{percent:.0}%");
                run_tool("pactl", &["set-sink-volume", "@DEFAULT_SINK@", level.as_str()])?;
            }
            println!("{percent:.0}%");
            Ok(())
        }
        "mute" | "unmute" | "toggle" => {
            if exec::have("wpctl") {
                let argument = match action {
                    "mute" => "1",
                    "unmute" => "0",
                    _ => "toggle",
                };
                run_tool("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", argument])?;
            } else {
                let argument = match action {
                    "mute" => "1",
                    "unmute" => "0",
                    _ => "toggle",
                };
                run_tool("pactl", &["set-sink-mute", "@DEFAULT_SINK@", argument])?;
            }
            let (percent, muted) = volume_state()?;
            println!("{percent:.0}%{}", if muted { " (muted)" } else { "" });
            Ok(())
        }
        other => Err(format!(
            "unknown volume action `{other}` (get, set, mute, unmute, toggle)"
        )),
    }
}

fn split_terse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == ':' {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
}

fn nmcli(args: &[&str]) -> Result<exec::Output, String> {
    if !exec::have("nmcli") {
        return Err("NetworkManager (nmcli) is not installed".into());
    }
    let output = exec::run("nmcli", args).map_err(|error| error.to_string())?;
    if !output.success() {
        return Err(output.failure_message());
    }
    Ok(output)
}

fn cmd_network(args: &Args) -> CmdResult {
    let action = args.word(0).unwrap_or("status");
    match action {
        "status" => {
            let devices = nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device", "status"])?;
            let radio = exec::run("nmcli", &["radio", "wifi"])
                .map(|output| output.stdout_trimmed().to_string())
                .unwrap_or_default();
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_object(None);
                w.str_field("wifi_radio", &radio);
                w.begin_array(Some("devices"));
                for line in devices.stdout.lines() {
                    let fields = split_terse(line);
                    if fields.len() < 4 {
                        continue;
                    }
                    w.begin_object(None);
                    w.str_field("device", &fields[0]);
                    w.str_field("type", &fields[1]);
                    w.str_field("state", &fields[2]);
                    w.str_field("connection", &fields[3]);
                    w.end_object();
                }
                w.end_array();
                w.end_object();
                println!("{}", w.finish());
            } else {
                println!("wifi radio: {radio}");
                for line in devices.stdout.lines() {
                    let fields = split_terse(line);
                    if fields.len() >= 4 && !fields[0].starts_with("lo") {
                        println!("{:<12} {:<10} {:<20} {}", fields[0], fields[1], fields[2], fields[3]);
                    }
                }
            }
            Ok(())
        }
        "wifi-list" => {
            let output = nmcli(&[
                "-t",
                "-f",
                "SSID,SIGNAL,SECURITY",
                "device",
                "wifi",
                "list",
                "--rescan",
                "no",
            ])?;
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_array(None);
                for line in output.stdout.lines() {
                    let fields = split_terse(line);
                    if fields.len() < 3 || fields[0].is_empty() {
                        continue;
                    }
                    w.begin_object(None);
                    w.str_field("ssid", &fields[0]);
                    w.uint_field("signal", fields[1].parse().unwrap_or(0));
                    w.str_field("security", &fields[2]);
                    w.end_object();
                }
                w.end_array();
                println!("{}", w.finish());
            } else {
                for line in output.stdout.lines() {
                    let fields = split_terse(line);
                    if fields.len() >= 3 && !fields[0].is_empty() {
                        println!("{:<32} {:>3}%  {}", fields[0], fields[1], fields[2]);
                    }
                }
            }
            Ok(())
        }
        "wifi-connect" => {
            let ssid = args
                .word(1)
                .ok_or_else(|| "usage: unify-ctl network wifi-connect <ssid> [password]".to_string())?;
            let mut arguments = vec!["device", "wifi", "connect", ssid];
            let password = args.word(2);
            if let Some(password) = password {
                arguments.push("password");
                arguments.push(password);
            }
            let output = nmcli(&arguments)?;
            println!("{}", output.stdout_trimmed());
            Ok(())
        }
        "wifi-disconnect" => {
            let status = nmcli(&["-t", "-f", "DEVICE,TYPE", "device", "status"])?;
            let device = status
                .stdout
                .lines()
                .map(split_terse)
                .find(|fields| fields.len() >= 2 && fields[1] == "wifi")
                .map(|fields| fields[0].clone())
                .ok_or_else(|| "no Wi-Fi device found".to_string())?;
            let output = nmcli(&["device", "disconnect", &device])?;
            println!("{}", output.stdout_trimmed());
            Ok(())
        }
        "enable" => run_tool("nmcli", &["radio", "wifi", "on"]),
        "disable" => run_tool("nmcli", &["radio", "wifi", "off"]),
        other => Err(format!(
            "unknown network action `{other}` (status, wifi-list, wifi-connect, wifi-disconnect, enable, disable)"
        )),
    }
}

fn bluetooth_show() -> Result<String, String> {
    if !exec::have("bluetoothctl") {
        return Err("BlueZ (bluetoothctl) is not installed".into());
    }
    let output = exec::run("bluetoothctl", &["show"]).map_err(|error| error.to_string())?;
    if !output.success() {
        return Err(output.failure_message());
    }
    Ok(output.stdout)
}

fn cmd_bluetooth(args: &Args) -> CmdResult {
    let action = args.word(0).unwrap_or("status");
    match action {
        "status" => {
            let text = bluetooth_show()?;
            let powered = text
                .lines()
                .find_map(|line| line.trim().strip_prefix("Powered:"))
                .map(|value| value.trim() == "yes")
                .unwrap_or(false);
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_object(None);
                w.bool_field("powered", powered);
                for line in text.lines() {
                    if let Some(address) = line.trim().strip_prefix("Controller ") {
                        if let Some(address) = address.split_whitespace().next() {
                            w.str_field("controller", address);
                        }
                    }
                }
                w.end_object();
                println!("{}", w.finish());
            } else {
                println!("bluetooth powered: {}", if powered { "yes" } else { "no" });
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Controller ") || trimmed.starts_with("Name:") {
                        println!("{trimmed}");
                    }
                }
            }
            Ok(())
        }
        "devices" => {
            let output = exec::run("bluetoothctl", &["devices"]).map_err(|e| e.to_string())?;
            if !output.success() {
                return Err(output.failure_message());
            }
            if args.json {
                let mut w = JsonWriter::compact();
                w.begin_array(None);
                for line in output.stdout.lines() {
                    let fields: Vec<&str> = line.splitn(3, ' ').collect();
                    if fields.len() < 3 || fields[0] != "Device" {
                        continue;
                    }
                    w.begin_object(None);
                    w.str_field("address", fields[1]);
                    w.str_field("name", fields[2]);
                    w.end_object();
                }
                w.end_array();
                println!("{}", w.finish());
            } else {
                print!("{}", output.stdout);
            }
            Ok(())
        }
        "scan" => {
            if !exec::have("bluetoothctl") {
                return Err("BlueZ (bluetoothctl) is not installed".into());
            }
            let output = exec::run("bluetoothctl", &["--timeout", "8", "scan", "on"])
                .map_err(|error| error.to_string())?;
            println!("{}", output.stdout_trimmed());
            Ok(())
        }
        "connect" => {
            let address = args
                .word(1)
                .ok_or_else(|| "usage: unify-ctl bluetooth connect <mac>".to_string())?;
            run_tool("bluetoothctl", &["connect", address])
        }
        "power" => {
            let state = args
                .word(1)
                .ok_or_else(|| "usage: unify-ctl bluetooth power on|off".to_string())?;
            if state != "on" && state != "off" {
                return Err("usage: unify-ctl bluetooth power on|off".into());
            }
            run_tool("bluetoothctl", &["power", state])
        }
        other => Err(format!(
            "unknown bluetooth action `{other}` (status, devices, scan, connect, power)"
        )),
    }
}

fn cmd_display(json: bool) -> CmdResult {
    if exec::have("wlr-randr") {
        if let Ok(output) = exec::run("wlr-randr", &["--json"]) {
            if output.success() {
                let trimmed = output.stdout_trimmed();
                if trimmed.starts_with('[') || trimmed.starts_with('{') {
                    println!("{trimmed}");
                    return Ok(());
                }
            }
        }
    }
    let mut connectors = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.contains('-') {
                continue;
            }
            let status = ufs::read_trimmed(&entry.path().join("status")).unwrap_or_default();
            let enabled = ufs::read_trimmed(&entry.path().join("enabled")).unwrap_or_default();
            let modes = ufs::read_trimmed(&entry.path().join("modes")).unwrap_or_default();
            connectors.push((name, status, enabled, modes));
        }
    }
    connectors.sort_by(|a, b| a.0.cmp(&b.0));
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for (name, status, enabled, modes) in &connectors {
            w.begin_object(None);
            w.str_field("name", name);
            w.str_field("status", status);
            w.str_field("enabled", enabled);
            w.begin_array(Some("modes"));
            for mode in modes.lines() {
                w.str_value(mode.trim());
            }
            w.end_array();
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else {
        for (name, status, enabled, modes) in &connectors {
            let first_mode = modes.lines().next().unwrap_or("unknown");
            println!("{name}: {status} (enabled: {enabled}) preferred {first_mode}");
        }
    }
    Ok(())
}

fn cmd_processes(args: &Args) -> CmdResult {
    let sort = args.flag_value("--sort").unwrap_or("cpu");
    let limit = args
        .flag_value("--limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);

    let first_cpu = procfs::cpu().map_err(|error| error.to_string())?;
    let first = procfs::processes(Path::new("/proc")).map_err(|error| error.to_string())?;
    sleep(Duration::from_millis(400));
    let second_cpu = procfs::cpu().map_err(|error| error.to_string())?;
    let second = procfs::processes(Path::new("/proc")).map_err(|error| error.to_string())?;

    let total_delta = second_cpu
        .total
        .total()
        .saturating_sub(first_cpu.total.total())
        .max(1);
    let cores = second_cpu.core_count().max(1) as f64;

    let mut rows: Vec<(u32, String, f64, u64, u64, String)> = Vec::new();
    for process in &second {
        let previous_ticks = first
            .iter()
            .find(|older| older.pid == process.pid)
            .map(|older| older.cpu_ticks)
            .unwrap_or(process.cpu_ticks);
        let delta = process.cpu_ticks.saturating_sub(previous_ticks);
        let cpu_percent = (delta as f64 / total_delta as f64) * cores * 100.0;
        rows.push((
            process.pid,
            process.name.clone(),
            cpu_percent,
            process.rss_kb,
            process.threads,
            process.cmdline.clone(),
        ));
    }
    if sort == "mem" {
        rows.sort_by(|a, b| b.3.cmp(&a.3));
    } else {
        rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    }
    rows.truncate(limit);

    if args.json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for (pid, name, cpu, rss, threads, cmdline) in &rows {
            w.begin_object(None);
            w.uint_field("pid", *pid as u64);
            w.str_field("name", name);
            w.num_field("cpu_percent", *cpu);
            w.uint_field("rss_kb", *rss);
            w.uint_field("threads", *threads);
            w.str_field("cmdline", cmdline);
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else {
        println!("{:>7}  {:>6}  {:>8}  {:>5}  {}", "PID", "CPU%", "RSS", "THR", "COMMAND");
        for (pid, name, cpu, rss, threads, _) in &rows {
            println!(
                "{pid:>7}  {cpu:>6.1}  {:>7.1}M  {threads:>5}  {name}",
                *rss as f64 / 1024.0
            );
        }
    }
    Ok(())
}
