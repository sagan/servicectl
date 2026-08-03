use anyhow::{bail, Result};
use std::path::PathBuf;

use super::start_stop_daemon::StartStopDaemonKind;
use super::{split_command, write_file_with_mode, InitSystem, ServiceInstaller};
use crate::service::ServiceUnit;

pub struct OpenRCInstaller {
    base_dir: PathBuf,
}

impl OpenRCInstaller {
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

        let (command, command_args) = split_command(exec_start);

        let mut lines = Vec::new();
        lines.push("#!/sbin/openrc-run".to_string());
        lines.push(format!("name=\"{}\"", service_name));
        if let Some(desc) = &unit.unit.description {
            lines.push(format!("description=\"{}\"", desc));
        }

        lines.push(format!("command=\"{}\"", command));
        if !command_args.is_empty() {
            lines.push(format!("command_args=\"{}\"", command_args));
        }

        if unit.service.user.is_some() || unit.service.group.is_some() {
            let u_str = unit.service.user.as_deref();
            let g_str = unit.service.group.as_deref();

            match (u_str, g_str) {
                (Some(u), Some(g)) => lines.push(format!("command_user=\"{}:{}\"", u, g)),
                (Some(u), None) => lines.push(format!("command_user=\"{}\"", u)),
                (None, Some(g)) => lines.push(format!("command_user=\":{}\"", g)),
                (None, None) => {}
            }

            let ssd = StartStopDaemonKind::detect();
            if let Some(ssd_args) = ssd.build_args_string(u_str, g_str) {
                lines.push(format!("start_stop_daemon_args=\"{}\"", ssd_args));
            }
        }

        if let Some(dir) = &unit.service.working_directory {
            lines.push(format!("directory=\"{}\"", dir.display()));
        }

        // Supervisor configuration
        lines.push("supervisor=\"supervise-daemon\"".to_string());
        if let Some(sec) = unit.service.restart_sec {
            lines.push(format!("respawn_delay={}", sec));
        }
        if let Some(pid_file) = &unit.service.pid_file {
            lines.push(format!("pidfile=\"{}\"", pid_file.display()));
        }

        lines.push(String::new());

        // Dynamic EnvironmentFile loading at runtime
        for env_file in &unit.service.environment_files {
            lines.push(format!(
                "if [ -f \"{}\" ]; then",
                env_file.path.display()
            ));
            lines.push("    set -o allexport".to_string());
            lines.push(format!("    . \"{}\"", env_file.path.display()));
            lines.push("    set +o allexport".to_string());
            lines.push("fi".to_string());
        }

        // Explicit Environment variables
        for (k, v) in &unit.service.environments {
            lines.push(format!("export {}=\"{}\"", k, v));
        }

        lines.push(String::new());

        // Dependencies
        let mut needs: Vec<String> = Vec::new();
        let mut uses: Vec<String> = Vec::new();
        let mut afters: Vec<String> = Vec::new();

        let all_deps = unit
            .unit
            .after
            .iter()
            .chain(unit.unit.requires.iter())
            .chain(unit.unit.wants.iter());

        for dep in all_deps {
            if dep.starts_with("network") {
                needs.push("net".to_string());
            } else if dep.starts_with("syslog") {
                needs.push("logger".to_string());
            } else {
                let dep_name = dep.trim_end_matches(".service").trim_end_matches(".target");
                if !dep_name.is_empty() {
                    afters.push(dep_name.to_string());
                }
            }
        }

        if !needs.is_empty() || !uses.is_empty() || !afters.is_empty() {
            lines.push("depend() {".to_string());
            if !needs.is_empty() {
                needs.dedup();
                lines.push(format!("    need {}", needs.join(" ")));
            }
            if !uses.is_empty() {
                uses.dedup();
                lines.push(format!("    use {}", uses.join(" ")));
            }
            if !afters.is_empty() {
                afters.dedup();
                lines.push(format!("    after {}", afters.join(" ")));
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // ExecReload support
        if let Some(exec_reload) = &unit.service.exec_reload {
            lines.push("extra_started_commands=\"reload\"".to_string());
            lines.push(String::new());
            lines.push("reload() {".to_string());
            lines.push(format!("    ebegin \"Reloading {}\"", service_name));
            lines.push(format!("    {}", exec_reload));
            lines.push("    eend $?".to_string());
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // ExecStop support
        if let Some(exec_stop) = &unit.service.exec_stop {
            lines.push("stop_post() {".to_string());
            lines.push(format!("    {}", exec_stop));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        Ok(lines.join("\n"))
    }
}

impl ServiceInstaller for OpenRCInstaller {
    fn init_system(&self) -> InitSystem {
        InitSystem::OpenRC
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
            "Successfully added OpenRC service '{}' -> {}",
            service_name,
            target_path.display()
        );
        Ok(())
    }

    fn replace(&self, service_name: &str, unit: &ServiceUnit, _raw_content: &str) -> Result<()> {
        let target_path = self.target_file_path(service_name);
        let exists = target_path.exists();
        let script = Self::generate_script(service_name, unit)?;
        write_file_with_mode(&target_path, &script, 0o755)?;
        if exists {
            println!(
                "Successfully replaced OpenRC service '{}' -> {}",
                service_name,
                target_path.display()
            );
        } else {
            println!(
                "Successfully added OpenRC service '{}' -> {}",
                service_name,
                target_path.display()
            );
        }
        Ok(())
    }
}
