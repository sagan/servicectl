pub mod openrc;
pub mod procd;
pub mod start_stop_daemon;
pub mod systemd;

use anyhow::{bail, Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::service::ServiceUnit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    Systemd,
    OpenRC,
    Procd,
}

impl std::str::FromStr for InitSystem {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "systemd" => Ok(InitSystem::Systemd),
            "openrc" => Ok(InitSystem::OpenRC),
            "procd" | "openwrt" => Ok(InitSystem::Procd),
            _ => bail!("Unsupported init system '{}'. Supported: systemd, openrc, procd", s),
        }
    }
}

impl InitSystem {
    pub fn detect() -> Result<Self> {
        // 1. Check environment variable override
        if let Ok(env_val) = std::env::var("SERVICECTL_INIT_SYSTEM") {
            return env_val.parse();
        }

        // 2. Check PID 1 comm / exe
        if let Ok(comm) = std::fs::read_to_string("/proc/1/comm") {
            let comm = comm.trim();
            if comm == "systemd" {
                return Ok(InitSystem::Systemd);
            } else if comm == "init" || comm.contains("openrc") {
                return Ok(InitSystem::OpenRC);
            } else if comm == "procd" {
                return Ok(InitSystem::Procd);
            }
        }

        if let Ok(exe_target) = std::fs::read_link("/proc/1/exe") {
            let path_str = exe_target.to_string_lossy();
            if path_str.contains("systemd") {
                return Ok(InitSystem::Systemd);
            } else if path_str.contains("openrc") {
                return Ok(InitSystem::OpenRC);
            } else if path_str.contains("procd") {
                return Ok(InitSystem::Procd);
            }
        }

        // 3. Check filesystem markers
        if Path::new("/run/systemd/system").exists() {
            return Ok(InitSystem::Systemd);
        }
        if Path::new("/sbin/openrc-run").exists() || Path::new("/run/openrc").exists() {
            return Ok(InitSystem::OpenRC);
        }
        if Path::new("/sbin/procd").exists()
            || Path::new("/etc/rc.common").exists()
            || Path::new("/etc/openwrt_release").exists()
        {
            return Ok(InitSystem::Procd);
        }
        if Path::new("/etc/systemd/system").exists() {
            return Ok(InitSystem::Systemd);
        }

        bail!("Could not automatically detect init system. Please specify --target or SERVICECTL_INIT_SYSTEM.")
    }
}

pub trait ServiceInstaller {
    #[allow(dead_code)]
    fn init_system(&self) -> InitSystem;
    fn target_file_path(&self, service_name: &str) -> PathBuf;
    #[allow(dead_code)]
    fn exists(&self, service_name: &str) -> bool {
        self.target_file_path(service_name).exists()
    }
    fn add(&self, service_name: &str, unit: &ServiceUnit, raw_content: &str) -> Result<()>;
    fn replace(&self, service_name: &str, unit: &ServiceUnit, raw_content: &str) -> Result<()>;
}

pub fn get_installer(target: InitSystem, override_dir: Option<PathBuf>) -> Box<dyn ServiceInstaller> {
    match target {
        InitSystem::Systemd => Box::new(systemd::SystemdInstaller::new(override_dir)),
        InitSystem::OpenRC => Box::new(openrc::OpenRCInstaller::new(override_dir)),
        InitSystem::Procd => Box::new(procd::ProcdInstaller::new(override_dir)),
    }
}

pub fn write_file_with_mode(path: &Path, content: &str, mode: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write service file {}", path.display()))?;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)
        .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    Ok(())
}

pub fn split_command(cmd: &str) -> (String, String) {
    let trimmed = cmd.trim();
    if let Some((exe, args)) = trimmed.split_once(char::is_whitespace) {
        (exe.trim().to_string(), args.trim().to_string())
    } else {
        (trimmed.to_string(), String::new())
    }
}

pub fn resolve_user_name(user_or_uid: &str) -> Result<String> {
    let passwd_path = std::env::var_os("SERVICECTL_PASSWD_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/passwd"));
    resolve_user_name_from_file(user_or_uid, &passwd_path)
}

pub fn resolve_user_name_from_file(user_or_uid: &str, passwd_path: &Path) -> Result<String> {
    if !user_or_uid.chars().all(|c| c.is_ascii_digit()) || user_or_uid.is_empty() {
        return Ok(user_or_uid.to_string());
    }

    let target_uid: u32 = user_or_uid
        .parse()
        .with_context(|| format!("Invalid numeric UID '{}'", user_or_uid))?;

    let content = std::fs::read_to_string(passwd_path)
        .with_context(|| format!("Failed to read {}", passwd_path.display()))?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            let username = parts[0].trim();
            if let Ok(uid) = parts[2].trim().parse::<u32>() {
                if uid == target_uid {
                    return Ok(username.to_string());
                }
            }
        }
    }

    bail!(
        "User with UID '{}' not found in {}",
        user_or_uid,
        passwd_path.display()
    );
}

pub fn resolve_group_name(group_or_gid: &str) -> Result<String> {
    let group_path = std::env::var_os("SERVICECTL_GROUP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/group"));
    resolve_group_name_from_file(group_or_gid, &group_path)
}

pub fn resolve_group_name_from_file(group_or_gid: &str, group_path: &Path) -> Result<String> {
    if !group_or_gid.chars().all(|c| c.is_ascii_digit()) || group_or_gid.is_empty() {
        return Ok(group_or_gid.to_string());
    }

    let target_gid: u32 = group_or_gid
        .parse()
        .with_context(|| format!("Invalid numeric GID '{}'", group_or_gid))?;

    let content = std::fs::read_to_string(group_path)
        .with_context(|| format!("Failed to read {}", group_path.display()))?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            let groupname = parts[0].trim();
            if let Ok(gid) = parts[2].trim().parse::<u32>() {
                if gid == target_gid {
                    return Ok(groupname.to_string());
                }
            }
        }
    }

    bail!(
        "Group with GID '{}' not found in {}",
        group_or_gid,
        group_path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_user_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let passwd_path = temp_dir.path().join("passwd");
        let passwd_content = r#"root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
testuser:x:1001:1001:Test User:/home/testuser:/bin/sh
"#;
        std::fs::write(&passwd_path, passwd_content).unwrap();

        assert_eq!(
            resolve_user_name_from_file("1001", &passwd_path).unwrap(),
            "testuser"
        );
        assert_eq!(
            resolve_user_name_from_file("0", &passwd_path).unwrap(),
            "root"
        );
        assert_eq!(
            resolve_user_name_from_file("nobody", &passwd_path).unwrap(),
            "nobody"
        );
        assert!(resolve_user_name_from_file("9999", &passwd_path).is_err());
    }

    #[test]
    fn test_resolve_group_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let group_path = temp_dir.path().join("group");
        let group_content = r#"root:x:0:
daemon:x:1:
testgroup:x:1001:
"#;
        std::fs::write(&group_path, group_content).unwrap();

        assert_eq!(
            resolve_group_name_from_file("1001", &group_path).unwrap(),
            "testgroup"
        );
        assert_eq!(
            resolve_group_name_from_file("0", &group_path).unwrap(),
            "root"
        );
        assert_eq!(
            resolve_group_name_from_file("nogroup", &group_path).unwrap(),
            "nogroup"
        );
        assert!(resolve_group_name_from_file("9999", &group_path).is_err());
    }
}

