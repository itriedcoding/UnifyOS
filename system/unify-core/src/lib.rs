//! Shared UnifyOS system library.
//!
//! Everything in this crate is dependency-free (`std` only) so the same code can
//! be compiled for the live ISO, the installed system, and host-side unit tests.
//!
//! * [`exec`] – running helper programs and capturing output
//! * [`fs`] – filesystem statistics, mount table and block-device discovery
//! * [`hwmon`] – thermal, fan, battery and backlight data from sysfs
//! * [`json`] – small streaming JSON writer used by the control CLI and daemon
//! * [`procfs`] – CPU, memory, load, disk and process data from `/proc`

pub mod exec;
pub mod fs;
pub mod hwmon;
pub mod json;
pub mod procfs;

/// Runtime state directory shared between UnifyOS services (tmpfs).
pub const RUN_DIR: &str = "/run/unify";
/// System-wide UnifyOS configuration directory.
pub const CONFIG_DIR: &str = "/etc/unify";
/// Version of the system components, taken from Cargo metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
