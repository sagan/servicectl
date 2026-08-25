use anyhow::{bail, Result};
use std::path::PathBuf;

use super::{
    resolve_group_name, resolve_user_name, write_file_if_different, write_file_with_mode,
    InitSystem, ServiceInstaller,
};
use crate::service::ServiceUnit;

pub struct ProcdInstaller {
    base_dir: PathBuf,
}

impl ProcdInstaller {
    pub fn new(override_dir: Option<PathBuf>) -> Self {
        let base_dir = override_dir
            .or_else(|| std::env::var_os("SERVICECTL_INITD_DIR").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/etc/init.d"));
        Self { base_dir }
    }

    pub fn generate_script(service_name: &str, unit: &ServiceUnit) -> Result<String> {
        let exec_start = unit.service.exec_start.as_deref().ok_or_else(|| {
            anyhow::anyhow!("Service '{}' has no ExecStart defined", service_name)
        })?;

        let resolved_user = match unit.service.user.as_deref() {
            Some(u) => Some(resolve_user_name(u)?),
            None => None,
        };

        let resolved_group = match unit.service.group.as_deref() {
            Some(g) => Some(resolve_group_name(g)?),
            None => None,
        };

        let mut lines = Vec::new();
        lines.push("#!/bin/sh /etc/rc.common".to_string());
        lines.push(String::new());
        lines.push("START=99".to_string());
        lines.push("STOP=10".to_string());
        lines.push("USE_PROCD=1".to_string());
        lines.push(String::new());

        lines.push("start_service() {".to_string());
        lines.push(format!("    procd_open_instance \"{}\"", service_name));

        // Runtime EnvironmentFile loading
        for env_file in &unit.service.environment_files {
            lines.push(format!(
                "    if [ -f \"{}\" ]; then",
                env_file.path.display()
            ));
            lines.push("        set -o allexport".to_string());
            lines.push(format!("        . \"{}\"", env_file.path.display()));
            lines.push("        set +o allexport".to_string());
            lines.push("    fi".to_string());
        }

        // Default environment variables for procd when user is root or unset
        let is_root_or_unset = match resolved_user.as_deref() {
            None | Some("root") => true,
            _ => false,
        };

        let has_home = unit.service.environments.iter().any(|(k, _)| k == "HOME");
        let has_user = unit.service.environments.iter().any(|(k, _)| k == "USER");

        if is_root_or_unset {
            if !has_home {
                lines.push("    export HOME=\"/root\"".to_string());
                lines.push("    procd_append_param env HOME=\"/root\"".to_string());
            }
            if !has_user {
                lines.push("    export USER=\"root\"".to_string());
                lines.push("    procd_append_param env USER=\"root\"".to_string());
            }
        }

        // Explicit Environment variables
        for (k, v) in &unit.service.environments {
            lines.push(format!("    export {}=\"{}\"", k, v));
            lines.push(format!("    procd_append_param env {}=\"{}\"", k, v));
        }

        lines.push(format!("    procd_set_param command {}", exec_start));
        lines.push("    procd_set_param stdout 1".to_string());
        lines.push("    procd_set_param stderr 1".to_string());

        if let Some(user) = &resolved_user {
            lines.push(format!("    procd_set_param user \"{}\"", user));
        }
        if let Some(group) = &resolved_group {
            lines.push(format!("    procd_set_param group \"{}\"", group));
        }
        if let Some(dir) = &unit.service.working_directory {
            lines.push(format!("    procd_set_param workdir \"{}\"", dir.display()));
        }

        let should_respawn = unit.service.restart.as_deref().map_or(false, |r| {
            r != "no" && r != "false" && r != "0"
        });

        if should_respawn {
            if let Some(sec) = unit.service.restart_sec {
                lines.push(format!("    procd_set_param respawn 3600 {} 0", sec));
            } else {
                lines.push("    procd_set_param respawn".to_string());
            }
        }

        lines.push("    procd_close_instance".to_string());
        lines.push("}".to_string());
        lines.push(String::new());

        if let Some(exec_reload) = &unit.service.exec_reload {
            lines.push("reload_service() {".to_string());
            lines.push(format!("    {}", exec_reload));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        if let Some(exec_stop) = &unit.service.exec_stop {
            lines.push("stop_service() {".to_string());
            lines.push(format!("    {}", exec_stop));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        Ok(lines.join("\n"))
    }
}

impl ServiceInstaller for ProcdInstaller {
    fn init_system(&self) -> InitSystem {
        InitSystem::Procd
    }

    fn target_file_path(&self, service_name: &str) -> PathBuf {
        self.base_dir.join(service_name)
    }

    fn add(&self, service_name: &str, unit: &ServiceUnit, _raw_content: &str) -> Result<()> {
        let target_path = self.target_file_path(service_name);
        if target_path.exists() {
            bail!(
                "Service '{}' already exists at {}",
                service_name,
                target_path.display()
            );
        }
        let script = Self::generate_script(service_name, unit)?;
        write_file_with_mode(&target_path, &script, 0o755)?;
        println!(
            "Successfully added Procd service '{}' -> {}",
            service_name,
            target_path.display()
        );
        Ok(())
    }

    fn replace(&self, service_name: &str, unit: &ServiceUnit, _raw_content: &str) -> Result<()> {
        let target_path = self.target_file_path(service_name);
        let exists = target_path.exists();
        let script = Self::generate_script(service_name, unit)?;
        write_file_if_different(&target_path, &script, 0o755)?;
        if exists {
            println!(
                "Successfully replaced Procd service '{}' -> {}",
                service_name,
                target_path.display()
            );
        } else {
            println!(
                "Successfully added Procd service '{}' -> {}",
                service_name,
                target_path.display()
            );
        }
        Ok(())
    }
}
