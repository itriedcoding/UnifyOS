//! Filesystem statistics, mount table and block-device discovery.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::raw::{c_char, c_int};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Mirrors glibc's `struct statvfs` layout on 64-bit Linux.
#[repr(C)]
struct RawStatVfs {
    f_bsize: u64,
    f_frsize: u64,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_favail: u64,
    f_fsid: u64,
    f_flag: u64,
    f_namemax: u64,
    f_spare: [i32; 6],
}

extern "C" {
    fn statvfs(path: *const c_char, buf: *mut RawStatVfs) -> c_int;
}

/// Used/free space of a mounted filesystem, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    pub total: u64,
    pub free: u64,
    pub available: u64,
}

impl Space {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    pub fn used_percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.used() as f64) * 100.0 / (self.total as f64)
        }
    }
}

/// Query `statvfs(3)` for the filesystem containing `path`.
pub fn space(path: &Path) -> io::Result<Space> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))?;
    let mut raw = RawStatVfs {
        f_bsize: 0,
        f_frsize: 0,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_favail: 0,
        f_fsid: 0,
        f_flag: 0,
        f_namemax: 0,
        f_spare: [0; 6],
    };
    // SAFETY: `c_path` is a valid NUL-terminated C string and `raw` is a valid,
    // correctly sized `struct statvfs` for the target platform.
    let rc = unsafe { statvfs(c_path.as_ptr(), &mut raw) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let block = if raw.f_frsize > 0 {
        raw.f_frsize
    } else {
        raw.f_bsize
    };
    Ok(Space {
        total: raw.f_blocks.saturating_mul(block),
        free: raw.f_bfree.saturating_mul(block),
        available: raw.f_bavail.saturating_mul(block),
    })
}

/// One entry of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub options: String,
}

pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    let mut mounts = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(separator) = fields.iter().position(|field| *field == "-") else {
            continue;
        };
        if fields.len() < 5 || separator + 2 >= fields.len() {
            continue;
        }
        mounts.push(Mount {
            source: fields[separator + 2].to_string(),
            target: fields[4].replace("\\040", " "),
            fstype: fields[separator + 1].to_string(),
            options: fields[5].to_string(),
        });
    }
    mounts
}

pub fn mounts() -> io::Result<Vec<Mount>> {
    Ok(parse_mountinfo(&fs::read_to_string(
        "/proc/self/mountinfo",
    )?))
}

/// A block device as listed in `/sys/block`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    pub name: String,
    pub size_bytes: u64,
    pub removable: bool,
    pub rotational: bool,
    pub model: String,
    pub partitions: Vec<String>,
}

/// Enumerate block devices (including NVMe, virtio, USB and LVM devices).
pub fn block_devices() -> io::Result<Vec<BlockDevice>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir("/sys/block")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();
        let sectors = read_u64(&base.join("size")).unwrap_or(0);
        let mut partitions = Vec::new();
        if let Ok(children) = fs::read_dir(&base) {
            for child in children.flatten() {
                let partition = child.file_name().to_string_lossy().to_string();
                if partition.len() > name.len()
                    && partition.starts_with(&name)
                    && child.path().join("partition").exists()
                {
                    partitions.push(partition);
                }
            }
        }
        partitions.sort();
        devices.push(BlockDevice {
            name,
            size_bytes: sectors.saturating_mul(512),
            removable: read_u64(&base.join("removable")) == Some(1),
            rotational: read_u64(&base.join("queue/rotational")) == Some(1),
            model: read_trimmed(&base.join("device/model")).unwrap_or_default(),
            partitions,
        });
    }
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(devices)
}

/// Read a sysfs/procfs file and trim surrounding whitespace.
pub fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| contents.trim().to_string())
}

pub fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

pub fn read_f64(path: &Path) -> Option<f64> {
    read_trimmed(path)?.parse().ok()
}

/// Write a single value to a sysfs attribute (brightness, LED, etc.).
pub fn write_attribute(path: &Path, value: &str) -> io::Result<()> {
    fs::write(path, value)
}

/// Locate an executable for optional dependencies (used by the desktop apps).
pub fn exists(path: &str) -> bool {
    PathBuf::from(path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mountinfo_entries() {
        let text = "\
36 35 98:0 / / rw,noatime master:1 - ext4 /dev/root rw,errors=remount-ro\n\
37 36 0:31 / /run rw,nosuid,nodev - tmpfs tmpfs rw,size=204800k\n\
38 36 8:1 / /boot/efi rw,relatime shared:2 - vfat /dev/sda1 rw,fmask=0077\n\
39 36 0:40 / /media/My\\040Disk rw - ext4 /dev/sdb1 rw\n";
        let mounts = parse_mountinfo(text);
        assert_eq!(mounts.len(), 4);
        assert_eq!(mounts[0].target, "/");
        assert_eq!(mounts[0].fstype, "ext4");
        assert_eq!(mounts[0].source, "/dev/root");
        assert_eq!(mounts[1].target, "/run");
        assert_eq!(mounts[2].target, "/boot/efi");
        assert_eq!(mounts[3].target, "/media/My Disk");
        assert_eq!(mounts[3].source, "/dev/sdb1");
    }

    #[test]
    fn ignores_malformed_mountinfo_lines() {
        let mounts = parse_mountinfo("garbage without separator\n\n");
        assert!(mounts.is_empty());
    }

    #[test]
    fn reports_space_for_root_filesystem() {
        let space = space(Path::new("/")).expect("statvfs on /");
        assert!(space.total > 0);
        assert!(space.free <= space.total);
        assert!(space.used_percent() >= 0.0 && space.used_percent() <= 100.0);
    }

    #[test]
    fn missing_path_is_an_error() {
        let error = space(Path::new("/definitely/not/here")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn reads_trimmed_values() {
        let dir = std::env::temp_dir().join(format!("unify-fs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("value");
        fs::write(&file, "  42\n").unwrap();
        assert_eq!(read_trimmed(&file).as_deref(), Some("42"));
        assert_eq!(read_u64(&file), Some(42));
        let _ = fs::remove_dir_all(&dir);
    }
}
