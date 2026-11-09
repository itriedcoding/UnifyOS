//! `unify-pkg` — package operations with UnifyOS policy.
//!
//! A thin, auditable layer over apt/dpkg that Unify Store and Unify Updater
//! call (through `pkexec` for privileged actions). Beyond plain passthrough it
//! enforces the policies that keep an UnifyOS installation bootable:
//!
//! * mutating operations require effective uid 0;
//! * essential and `Priority: required` packages cannot be removed;
//! * every mutation is logged to `/var/log/unify/pkg.log`;
//! * machine-readable output via `--json`.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use unify_core::exec;
use unify_core::json::JsonWriter;

extern "C" {
    fn geteuid() -> u32;
}

const LOG_PATH: &str = "/var/log/unify/pkg.log";

fn main() {
    let mut json = false;
    let mut words: Vec<String> = Vec::new();
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--json" | "-j" => json = true,
            other => words.push(other.to_string()),
        }
    }
    let command = words.first().cloned().unwrap_or_default();
    let rest: Vec<String> = if words.is_empty() {
        Vec::new()
    } else {
        words[1..].to_vec()
    };

    let outcome = match command.as_str() {
        "search" => cmd_search(&rest, json),
        "info" => cmd_info(&rest, json),
        "installed" => cmd_installed(&rest, json),
        "upgradable" => cmd_upgradable(json),
        "install" | "remove" | "upgrade" | "autoremove" => cmd_mutate(&command, &rest, json),
        "refresh" => cmd_refresh(json),
        "check" => cmd_check(json),
        "version" => {
            println!("unify-pkg {}", unify_core::VERSION);
            Ok(())
        }
        "help" | "" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`; run `unify-pkg help`")),
    };

    if let Err(message) = outcome {
        eprintln!("unify-pkg: {message}");
        std::process::exit(1);
    }
}

fn usage() {
    println!(
        "unify-pkg {version} — UnifyOS package operations\n\
\n\
Queries\n\
  search <term> [--json]      search available packages\n\
  info <package> [--json]     show package metadata\n\
  installed [filter]          list installed packages\n\
  upgradable [--json]         list pending upgrades\n\
  check [--json]              audit the package database\n\
\n\
Mutations (require root; Unify Store and Updater use pkexec)\n\
  refresh                     update package lists\n\
  install <package...>        install packages\n\
  remove <package...>         remove packages (essential packages refused)\n\
  upgrade                     upgrade all installed packages\n\
  autoremove                  remove orphaned dependencies\n",
        version = unify_core::VERSION
    );
}

fn require_root() -> Result<(), String> {
    // SAFETY: geteuid has no preconditions and no side effects.
    if unsafe { geteuid() } != 0 {
        return Err(
            "this operation requires administrator rights; Unify Store and Unify Updater \
             escalate through pkexec automatically"
                .to_string(),
        );
    }
    Ok(())
}

fn apt_run(args: &[&str]) -> Result<exec::Output, String> {
    let output = Command::new("apt-get")
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run apt-get: {error}"))?;
    Ok(exec::Output {
        program: "apt-get".to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn log_action(action: &str, packages: &[String], success: bool, detail: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if let Some(parent) = std::path::Path::new(LOG_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let summary: String = detail.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        let _ = writeln!(
            file,
            "{timestamp}\t{action}\t{}\t{}\t{summary}",
            if success { "ok" } else { "FAILED" },
            packages.join(",")
        );
    }
}

fn installed_packages() -> Result<Vec<(String, String, u64)>, String> {
    let output = exec::run(
        "dpkg-query",
        &[
            "-W",
            "-f=${Package}\\t${Version}\\t${Installed-Size}\\t${Status}\\n",
        ],
    )
    .map_err(|error| format!("cannot run dpkg-query: {error}"))?;
    if !output.success() {
        return Err(output.failure_message());
    }
    let mut packages = Vec::new();
    for line in output.stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 || !fields[3].ends_with(" installed") {
            continue;
        }
        packages.push((
            fields[0].to_string(),
            fields[1].to_string(),
            fields[2].parse().unwrap_or(0),
        ));
    }
    Ok(packages)
}

fn cmd_installed(rest: &[String], json: bool) -> Result<(), String> {
    let filter = rest.first().map(|value| value.to_lowercase());
    let mut packages = installed_packages()?;
    if let Some(filter) = &filter {
        packages.retain(|(name, _, _)| name.to_lowercase().contains(filter));
    }
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for (name, version, size_kb) in &packages {
            w.begin_object(None);
            w.str_field("package", name);
            w.str_field("version", version);
            w.uint_field("installed_kb", *size_kb);
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else {
        for (name, version, size_kb) in &packages {
            println!("{name}\t{version}\t{size_kb} KiB");
        }
        eprintln!("{} package(s) installed", packages.len());
    }
    Ok(())
}

fn upgradable_packages() -> Result<Vec<(String, String, String)>, String> {
    let output = apt_run(&["-s", "-q", "dist-upgrade"])?;
    if !output.success() {
        return Err(output.failure_message());
    }
    let mut upgrades = Vec::new();
    for line in output.stdout.lines() {
        let Some(rest) = line.strip_prefix("Inst ") else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let installed = fields
            .next()
            .unwrap_or("")
            .trim_matches(|ch| ch == '[' || ch == ']')
            .to_string();
        let available = fields
            .next()
            .unwrap_or("")
            .trim_start_matches('(')
            .to_string();
        upgrades.push((name.to_string(), installed, available));
    }
    Ok(upgrades)
}

fn cmd_upgradable(json: bool) -> Result<(), String> {
    let upgrades = upgradable_packages()?;
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for (name, installed, available) in &upgrades {
            w.begin_object(None);
            w.str_field("package", name);
            w.str_field("installed_version", installed);
            w.str_field("available_version", available);
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else if upgrades.is_empty() {
        println!("UnifyOS is up to date");
    } else {
        for (name, installed, available) in &upgrades {
            println!("{name}\t{installed} -> {available}");
        }
        eprintln!("{} upgrade(s) available", upgrades.len());
    }
    Ok(())
}

fn cmd_search(rest: &[String], json: bool) -> Result<(), String> {
    let term = rest
        .first()
        .ok_or_else(|| "usage: unify-pkg search <term>".to_string())?;
    let output = exec::run("apt-cache", &["search", "--names-only", term])
        .map_err(|error| format!("cannot run apt-cache: {error}"))?;
    if !output.success() {
        return Err(output.failure_message());
    }
    let mut results = Vec::new();
    for line in output.stdout.lines() {
        if let Some((name, description)) = line.split_once(" - ") {
            results.push((name.trim().to_string(), description.trim().to_string()));
        }
    }
    if json {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        for (name, description) in &results {
            w.begin_object(None);
            w.str_field("package", name);
            w.str_field("description", description);
            w.end_object();
        }
        w.end_array();
        println!("{}", w.finish());
    } else {
        for (name, description) in &results {
            println!("{name} - {description}");
        }
    }
    Ok(())
}

fn cmd_info(rest: &[String], json: bool) -> Result<(), String> {
    let package = rest
        .first()
        .ok_or_else(|| "usage: unify-pkg info <package>".to_string())?;
    let output = exec::run("apt-cache", &["show", package])
        .map_err(|error| format!("cannot run apt-cache: {error}"))?;
    if !output.success() || output.stdout_trimmed().is_empty() {
        return Err(format!("package `{package}` was not found in the package lists"));
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in output.stdout.lines() {
        if line.starts_with("Description-en:") || line.starts_with("Description:") {
            let value = line.splitn(2, ':').nth(1).unwrap_or("").trim();
            fields.entry("description".to_string()).or_insert(value.to_string());
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            if !key.contains(' ') {
                fields
                    .entry(key.trim().to_lowercase())
                    .or_insert_with(|| value.trim().to_string());
            }
        }
    }
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.str_field("package", package);
        for (key, value) in &fields {
            w.str_field(key, value);
        }
        w.end_object();
        println!("{}", w.finish());
    } else {
        println!("{package}");
        for (key, value) in &fields {
            println!("  {key:<18} {value}");
        }
    }
    Ok(())
}

fn protected_packages(requested: &[String]) -> Result<Vec<String>, String> {
    let output = exec::run(
        "dpkg-query",
        &["-W", "-f=${Package}\\t${Essential}\\t${Priority}\\n"],
    )
    .map_err(|error| format!("cannot run dpkg-query: {error}"))?;
    let mut protected = Vec::new();
    for line in output.stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }
        let required = fields[1] == "yes" || fields[2] == "required";
        if required && requested.iter().any(|package| package == fields[0]) {
            protected.push(fields[0].to_string());
        }
    }
    Ok(protected)
}

fn cmd_mutate(action: &str, packages: &[String], json: bool) -> Result<(), String> {
    require_root()?;
    if packages.is_empty() && (action == "install" || action == "remove") {
        return Err(format!("usage: unify-pkg {action} <package...>"));
    }
    if action == "remove" {
        let protected = protected_packages(packages)?;
        if !protected.is_empty() {
            return Err(format!(
                "refusing to remove required or essential packages: {}",
                protected.join(", ")
            ));
        }
    }

    let mut args: Vec<String> = vec![
        "-y".to_string(),
        "--no-install-recommends".to_string(),
        "-o".to_string(),
        "Dpkg::Options::=--force-confold".to_string(),
    ];
    if action == "upgrade" {
        args.push("dist-upgrade".to_string());
    } else {
        args.push(action.to_string());
    }
    args.extend(packages.iter().cloned());

    let references: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = apt_run(&references)?;
    let combined = format!("{}{}", output.stdout, output.stderr);
    log_action(action, packages, output.success(), &combined);

    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.str_field("action", action);
        w.begin_array(Some("packages"));
        for package in packages {
            w.str_value(package);
        }
        w.end_array();
        w.bool_field("success", output.success());
        w.str_field("output", combined.trim());
        w.end_object();
        println!("{}", w.finish());
    } else {
        print!("{combined}");
    }
    if !output.success() {
        return Err(format!(
            "apt-get {action} failed; details were logged to {LOG_PATH}"
        ));
    }
    Ok(())
}

fn cmd_refresh(json: bool) -> Result<(), String> {
    require_root()?;
    let output = apt_run(&["update"])?;
    let combined = format!("{}{}", output.stdout, output.stderr);
    log_action("refresh", &[], output.success(), &combined);
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.bool_field("success", output.success());
        w.str_field("output", combined.trim());
        w.end_object();
        println!("{}", w.finish());
    } else {
        print!("{combined}");
    }
    if !output.success() {
        return Err("refreshing package lists failed".to_string());
    }
    Ok(())
}

fn cmd_check(json: bool) -> Result<(), String> {
    let audit = exec::run("dpkg", &["--audit"]).map_err(|error| error.to_string())?;
    let issues = audit.stdout.trim().to_string();
    let ok = audit.success() && issues.is_empty();
    if json {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.bool_field("ok", ok);
        w.str_field("issues", &issues);
        w.end_object();
        println!("{}", w.finish());
    } else if ok {
        println!("package database is consistent");
    } else {
        println!("package database reports issues:\n{issues}");
    }
    if !ok {
        return Err("package database is not consistent".to_string());
    }
    Ok(())
}
