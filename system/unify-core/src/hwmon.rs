//! Hardware sensors from sysfs: temperatures, fans, batteries and backlights.

use std::fs;
use std::path::Path;

use crate::fs::{read_f64, read_trimmed, read_u64};

pub const HWMON_ROOT: &str = "/sys/class/hwmon";
pub const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";
pub const BACKLIGHT_ROOT: &str = "/sys/class/backlight";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    Temperature,
    Fan,
    Voltage,
}

impl SensorKind {
    pub fn unit(self) -> &'static str {
        match self {
            SensorKind::Temperature => "°C",
            SensorKind::Fan => "RPM",
            SensorKind::Voltage => "V",
        }
    }
}

/// One readable sensor value.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub chip: String,
    pub label: String,
    pub kind: SensorKind,
    pub value: f64,
}

/// Split `temp1_input` into `("temp", 1)`; returns `None` for other attributes.
fn sensor_file_parts(name: &str) -> Option<(&str, u32)> {
    let base = name.strip_suffix("_input")?;
    let digit_start = base.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = base.split_at(digit_start);
    if prefix.is_empty() || digits.is_empty() {
        return None;
    }
    Some((prefix, digits.parse().ok()?))
}

/// Read every temperature/fan/voltage input below `root` (normally
/// `/sys/class/hwmon`).
pub fn read_sensors(root: &Path) -> Vec<Sensor> {
    let mut sensors = Vec::new();
    let Ok(chips) = fs::read_dir(root) else {
        return sensors;
    };
    for chip_entry in chips.flatten() {
        let chip_dir = chip_entry.path();
        let chip = read_trimmed(&chip_dir.join("name"))
            .unwrap_or_else(|| chip_entry.file_name().to_string_lossy().to_string());
        let Ok(files) = fs::read_dir(&chip_dir) else {
            continue;
        };
        for file in files.flatten() {
            let file_name = file.file_name().to_string_lossy().to_string();
            let Some((prefix, index)) = sensor_file_parts(&file_name) else {
                continue;
            };
            let kind = match prefix {
                "temp" => SensorKind::Temperature,
                "fan" => SensorKind::Fan,
                "in" => SensorKind::Voltage,
                _ => continue,
            };
            let Some(raw) = read_f64(&file.path()) else {
                continue;
            };
            let value = match kind {
                // sysfs reports millidegrees / millivolts.
                SensorKind::Temperature | SensorKind::Voltage => raw / 1000.0,
                SensorKind::Fan => raw,
            };
            let label = read_trimmed(&chip_dir.join(format!("{prefix}{index}_label")))
                .unwrap_or_else(|| chip.clone());
            sensors.push(Sensor {
                chip: chip.clone(),
                label,
                kind,
                value,
            });
        }
    }
    sensors.sort_by(|a, b| {
        (a.chip.as_str(), a.label.as_str()).cmp(&(b.chip.as_str(), b.label.as_str()))
    });
    sensors
}

pub fn sensors() -> Vec<Sensor> {
    read_sensors(Path::new(HWMON_ROOT))
}

/// A battery or UPS as reported by `/sys/class/power_supply`.
#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub name: String,
    pub present: bool,
    pub status: String,
    pub capacity_percent: Option<f64>,
    pub energy_now_wh: Option<f64>,
    pub energy_full_wh: Option<f64>,
    pub power_now_w: Option<f64>,
    pub voltage_v: Option<f64>,
}

impl Battery {
    pub fn is_charging(&self) -> bool {
        self.status.eq_ignore_ascii_case("charging")
    }

    pub fn is_discharging(&self) -> bool {
        self.status.eq_ignore_ascii_case("discharging")
    }

    /// Seconds of runtime left at the current discharge rate, when known.
    pub fn seconds_remaining(&self) -> Option<u64> {
        let energy = self.energy_now_wh?;
        let power = self.power_now_w?;
        if power <= 0.0 {
            return None;
        }
        Some(((energy / power) * 3600.0) as u64)
    }
}

/// Convert micro-amp-hours to watt-hours using the current voltage.
fn charge_to_wh(charge_uh: f64, voltage_uv: Option<f64>) -> Option<f64> {
    let volts = voltage_uv? / 1_000_000.0;
    Some(charge_uh / 1_000_000.0 * volts)
}

/// Some batteries report `energy_*` (µWh), others `charge_*` (µAh).
fn read_energy(dir: &Path, stem: &str, voltage_uv: Option<f64>) -> Option<f64> {
    if let Some(wh) = read_f64(&dir.join(stem)) {
        return Some(wh / 1_000_000.0);
    }
    let charge_stem = stem.replace("energy", "charge");
    let charge = read_f64(&dir.join(charge_stem))?;
    charge_to_wh(charge, voltage_uv)
}

pub fn read_batteries(root: &Path) -> Vec<Battery> {
    let mut batteries = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return batteries;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if read_trimmed(&dir.join("type")).as_deref() != Some("Battery") {
            continue;
        }
        let present = read_u64(&dir.join("present"))
            .map(|value| value != 0)
            .unwrap_or(true);
        let status = read_trimmed(&dir.join("status")).unwrap_or_else(|| "Unknown".to_string());
        let voltage_uv = read_f64(&dir.join("voltage_now"));
        batteries.push(Battery {
            name: entry.file_name().to_string_lossy().to_string(),
            present,
            status,
            capacity_percent: read_f64(&dir.join("capacity")),
            energy_now_wh: read_energy(&dir, "energy_now", voltage_uv),
            energy_full_wh: read_energy(&dir, "energy_full", voltage_uv),
            power_now_w: read_f64(&dir.join("power_now")).map(|value| value / 1_000_000.0),
            voltage_v: voltage_uv.map(|value| value / 1_000_000.0),
        });
    }
    batteries.sort_by(|a, b| a.name.cmp(&b.name));
    batteries
}

pub fn batteries() -> Vec<Battery> {
    read_batteries(Path::new(POWER_SUPPLY_ROOT))
}

/// A panel backlight device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlight {
    pub device: String,
    pub brightness: u64,
    pub max_brightness: u64,
}

impl Backlight {
    pub fn percent(&self) -> f64 {
        if self.max_brightness == 0 {
            0.0
        } else {
            (self.brightness as f64) * 100.0 / (self.max_brightness as f64)
        }
    }
}

pub fn read_backlights(root: &Path) -> Vec<Backlight> {
    let mut backlights = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return backlights;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(max_brightness) = read_u64(&dir.join("max_brightness")) else {
            continue;
        };
        backlights.push(Backlight {
            device: entry.file_name().to_string_lossy().to_string(),
            brightness: read_u64(&dir.join("brightness")).unwrap_or(0),
            max_brightness,
        });
    }
    backlights.sort_by(|a, b| a.device.cmp(&b.device));
    backlights
}

pub fn backlights() -> Vec<Backlight> {
    read_backlights(Path::new(BACKLIGHT_ROOT))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "unify-hwmon-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_temperatures_and_fans() {
        let root = fixture("sensors");
        let chip = root.join("hwmon0");
        fs::create_dir_all(&chip).unwrap();
        fs::write(chip.join("name"), "k10temp\n").unwrap();
        fs::write(chip.join("temp1_input"), "42500\n").unwrap();
        fs::write(chip.join("temp1_label"), "Tctl\n").unwrap();
        fs::write(chip.join("fan1_input"), "1200\n").unwrap();
        fs::write(chip.join("in0_input"), "1200\n").unwrap();

        let sensors = read_sensors(&root);
        assert_eq!(sensors.len(), 3);
        let temp = sensors
            .iter()
            .find(|sensor| sensor.kind == SensorKind::Temperature)
            .expect("temperature");
        assert_eq!(temp.label, "Tctl");
        assert!((temp.value - 42.5).abs() < 1e-9);
        let fan = sensors
            .iter()
            .find(|sensor| sensor.kind == SensorKind::Fan)
            .expect("fan");
        assert_eq!(fan.value, 1200.0);
        assert_eq!(fan.label, "k10temp");
        let voltage = sensors
            .iter()
            .find(|sensor| sensor.kind == SensorKind::Voltage)
            .expect("voltage");
        assert!((voltage.value - 1.2).abs() < 1e-9);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ignores_attributes_it_does_not_understand() {
        assert_eq!(sensor_file_parts("temp1_input"), Some(("temp", 1)));
        assert_eq!(sensor_file_parts("in0_input"), Some(("in", 0)));
        assert_eq!(sensor_file_parts("name"), None);
        assert_eq!(sensor_file_parts("temp_input"), None);
        assert_eq!(sensor_file_parts("power1_average"), None);
    }

    #[test]
    fn reads_battery_energy_and_runtime() {
        let root = fixture("battery");
        let battery = root.join("BAT0");
        fs::create_dir_all(&battery).unwrap();
        fs::write(battery.join("type"), "Battery\n").unwrap();
        fs::write(battery.join("status"), "Discharging\n").unwrap();
        fs::write(battery.join("capacity"), "76\n").unwrap();
        fs::write(battery.join("energy_now"), "38000000\n").unwrap();
        fs::write(battery.join("energy_full"), "50000000\n").unwrap();
        fs::write(battery.join("power_now"), "9500000\n").unwrap();
        fs::write(battery.join("voltage_now"), "11400000\n").unwrap();

        let batteries = read_batteries(&root);
        assert_eq!(batteries.len(), 1);
        let read = &batteries[0];
        assert_eq!(read.name, "BAT0");
        assert!(read.is_discharging());
        assert_eq!(read.capacity_percent, Some(76.0));
        assert!((read.energy_now_wh.unwrap() - 38.0).abs() < 1e-9);
        assert!((read.power_now_w.unwrap() - 9.5).abs() < 1e-9);
        assert!((read.voltage_v.unwrap() - 11.4).abs() < 1e-9);
        assert_eq!(read.seconds_remaining(), Some(14400));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn converts_charge_based_batteries_to_watt_hours() {
        let root = fixture("charge");
        let battery = root.join("BAT1");
        fs::create_dir_all(&battery).unwrap();
        fs::write(battery.join("type"), "Battery\n").unwrap();
        fs::write(battery.join("charge_now"), "2000000\n").unwrap();
        fs::write(battery.join("charge_full"), "4000000\n").unwrap();
        fs::write(battery.join("voltage_now"), "12000000\n").unwrap();

        let batteries = read_batteries(&root);
        assert_eq!(batteries.len(), 1);
        assert!((batteries[0].energy_now_wh.unwrap() - 24.0).abs() < 1e-9);
        assert!((batteries[0].energy_full_wh.unwrap() - 48.0).abs() < 1e-9);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_backlight_percentage() {
        let root = fixture("backlight");
        let panel = root.join("intel_backlight");
        fs::create_dir_all(&panel).unwrap();
        fs::write(panel.join("brightness"), "400\n").unwrap();
        fs::write(panel.join("max_brightness"), "1000\n").unwrap();

        let backlights = read_backlights(&root);
        assert_eq!(backlights.len(), 1);
        assert!((backlights[0].percent() - 40.0).abs() < 1e-9);
        let _ = fs::remove_dir_all(&root);
    }
}
