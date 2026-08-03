use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

mod adhoc;
mod service;
mod translator;

use service::ServiceUnit;
use translator::{get_installer, InitSystem};

#[derive(Parser)]
#[command(
    name = "servicectl",
    version,
    about = "CLI tool to manage and translate Linux services across Systemd, OpenRC, and Procd"
)]
struct Cli {
    /// Target init system to use (systemd, openrc, procd). Auto-detected if not specified.
    #[arg(long, global = true)]
    target: Option<InitSystem>,

    /// Override output directory for service installation
    #[arg(long, global = true)]
    sys_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new system service. Returns an error if the service already exists.
    Add {
        /// Path to systemd service file or ad-hoc command line
        input: String,

        /// Treat input as a direct command line instead of a service file path
        #[arg(long)]
        adhoc: bool,

        /// Override default service name
        #[arg(long)]
        name: Option<String>,
    },
    /// Add a new system service or replace an existing one.
    Replace {
        /// Path to systemd service file or ad-hoc command line
        input: String,

        /// Treat input as a direct command line instead of a service file path
        #[arg(long)]
        adhoc: bool,

        /// Override default service name
        #[arg(long)]
        name: Option<String>,
    },
}

fn infer_service_name(path: &Path) -> Result<String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid service file path '{}'", path.display()))?
        .to_string_lossy();

    let name = if let Some(stem) = file_name.strip_suffix(".service") {
        stem
    } else {
        file_name.as_ref()
    };

    if name.is_empty() {
        bail!("Could not infer service name from path '{}'", path.display());
    }

    Ok(name.to_string())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (input, adhoc, name_override, is_replace) = match cli.command {
        Commands::Add { input, adhoc, name } => (input, adhoc, name, false),
        Commands::Replace { input, adhoc, name } => (input, adhoc, name, true),
    };

    let (service_name, unit, raw_content) = if adhoc {
        let adhoc_res = adhoc::parse_adhoc_command(&input, name_override.as_deref())?;
        let raw = adhoc_res.unit.to_systemd_content();
        (adhoc_res.service_name, adhoc_res.unit, raw)
    } else {
        let service_file = PathBuf::from(&input);
        let raw = fs::read_to_string(&service_file)
            .with_context(|| format!("Failed to read service file {}", service_file.display()))?;
        let inferred = infer_service_name(&service_file)?;
        let service_name = name_override.unwrap_or(inferred);
        let unit = ServiceUnit::parse(&raw)
            .with_context(|| format!("Failed to parse service file {}", service_file.display()))?;
        (service_name, unit, raw)
    };

    let target = match cli.target {
        Some(t) => t,
        None => InitSystem::detect()?,
    };

    let installer = get_installer(target, cli.sys_dir);

    if is_replace {
        installer.replace(&service_name, &unit, &raw_content)?;
    } else {
        installer.add(&service_name, &unit, &raw_content)?;
    }

    Ok(())
}
