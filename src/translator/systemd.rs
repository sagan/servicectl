use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use super::{write_file_if_different, write_file_with_mode, InitSystem, ServiceInstaller};
use crate::service::ServiceUnit;

pub struct SystemdInstaller {
    base_dir: PathBuf,
}

impl SystemdInstaller {
    pub fn new(override_dir: Option<PathBuf>) -> Self {
        let base_dir = override_dir
            .or_else(|| std::env::var_os("SERVICECTL_SYSTEMD_DIR").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/etc/systemd/system"));
        Self { base_dir }
    }

    fn run_daemon_reload(&self) -> Result<()> {
        if std::env::var("SERVICECTL_SKIP_SYSTEMCTL").is_ok() {
            return Ok(());
        }

        match Command::new("systemctl").arg("daemon-reload").status() {
            Ok(status) => {
                if !status.success() {
                    bail!("'systemctl daemon-reload' failed with status: {}", status);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // systemctl is not installed or not in PATH
            }
            Err(e) => {
                return Err(e).context("Failed to execute 'systemctl daemon-reload'");
            }
        }
        Ok(())
    }
}

impl ServiceInstaller for SystemdInstaller {
    fn init_system(&self) -> InitSystem {
        InitSystem::Systemd
    }

    fn target_file_path(&self, service_name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.service", service_name))
    }

    fn add(&self, service_name: &str, _unit: &ServiceUnit, raw_content: &str) -> Result<()> {
        let target_path = self.target_file_path(service_name);
        if target_path.exists() {
            bail!(
                "Service '{}' already exists at {}",
                service_name,
                target_path.display()
            );
        }
        write_file_with_mode(&target_path, raw_content, 0o644)?;
        println!(
            "Successfully added systemd service '{}' -> {}",
            service_name,
            target_path.display()
        );
        self.run_daemon_reload()?;
        Ok(())
    }

    fn replace(&self, service_name: &str, _unit: &ServiceUnit, raw_content: &str) -> Result<()> {
        let target_path = self.target_file_path(service_name);
        let exists = target_path.exists();
        write_file_if_different(&target_path, raw_content, 0o644)?;
        if exists {
            println!(
                "Successfully replaced systemd service '{}' -> {}",
                service_name,
                target_path.display()
            );
        } else {
            println!(
                "Successfully added systemd service '{}' -> {}",
                service_name,
                target_path.display()
            );
        }
        self.run_daemon_reload()?;
        Ok(())
    }
}
