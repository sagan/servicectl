use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentFileRef {
    pub path: PathBuf,
    pub optional: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitSection {
    pub description: Option<String>,
    pub after: Vec<String>,
    pub requires: Vec<String>,
    pub wants: Vec<String>,
    pub before: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceSection {
    pub service_type: String, // "simple", "exec", "forking", "oneshot", etc.
    pub user: Option<String>,
    pub group: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub exec_start: Option<String>,
    pub exec_stop: Option<String>,
    pub exec_reload: Option<String>,
    pub restart: Option<String>,
    pub restart_sec: Option<u64>,
    pub environments: Vec<(String, String)>,
    pub environment_files: Vec<EnvironmentFileRef>,
    pub private_tmp: bool,
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallSection {
    pub wanted_by: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceUnit {
    pub unit: UnitSection,
    pub service: ServiceSection,
    pub install: InstallSection,
}

impl ServiceUnit {
    pub fn parse(content: &str) -> Result<Self> {
        let mut unit = ServiceUnit::default();
        unit.service.service_type = "simple".to_string();

        let mut current_section = "";

        let mut raw_lines = Vec::new();
        let mut current_line = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with('\\') {
                current_line.push_str(&trimmed[..trimmed.len() - 1]);
                current_line.push(' ');
            } else {
                current_line.push_str(line);
                raw_lines.push(current_line.clone());
                current_line.clear();
            }
        }
        if !current_line.is_empty() {
            raw_lines.push(current_line);
        }

        for line in &raw_lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let sec_name = &trimmed[1..trimmed.len() - 1];
                current_section = match sec_name {
                    "Unit" => "Unit",
                    "Service" => "Service",
                    "Install" => "Install",
                    _ => "",
                };
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim();
                let val = val.trim();

                match current_section {
                    "Unit" => match key {
                        "Description" => unit.unit.description = Some(val.to_string()),
                        "After" => unit.unit.after.extend(val.split_whitespace().map(String::from)),
                        "Requires" => unit.unit.requires.extend(val.split_whitespace().map(String::from)),
                        "Wants" => unit.unit.wants.extend(val.split_whitespace().map(String::from)),
                        "Before" => unit.unit.before.extend(val.split_whitespace().map(String::from)),
                        _ => {}
                    },
                    "Service" => match key {
                        "Type" => unit.service.service_type = val.to_string(),
                        "User" => unit.service.user = Some(val.to_string()),
                        "Group" => unit.service.group = Some(val.to_string()),
                        "WorkingDirectory" => {
                            unit.service.working_directory = Some(PathBuf::from(val))
                        }
                        "ExecStart" => unit.service.exec_start = Some(val.to_string()),
                        "ExecStop" => unit.service.exec_stop = Some(val.to_string()),
                        "ExecReload" => unit.service.exec_reload = Some(val.to_string()),
                        "Restart" => unit.service.restart = Some(val.to_string()),
                        "RestartSec" => {
                            let sec_str = val.trim_end_matches('s').trim_end_matches("sec");
                            if let Ok(sec) = sec_str.parse::<u64>() {
                                unit.service.restart_sec = Some(sec);
                            }
                        }
                        "PrivateTmp" => {
                            let lower = val.to_lowercase();
                            unit.service.private_tmp =
                                lower == "true" || lower == "yes" || lower == "1";
                        }
                        "PIDFile" => unit.service.pid_file = Some(PathBuf::from(val)),
                        "Environment" => {
                            let envs = parse_environment_line(val);
                            unit.service.environments.extend(envs);
                        }
                        "EnvironmentFile" => {
                            let (optional, path_str) = if let Some(stripped) = val.strip_prefix('-')
                            {
                                (true, stripped.trim())
                            } else {
                                (false, val)
                            };
                            unit.service.environment_files.push(EnvironmentFileRef {
                                path: PathBuf::from(path_str),
                                optional,
                            });
                        }
                        _ => {}
                    },
                    "Install" => match key {
                        "WantedBy" => unit
                            .install
                            .wanted_by
                            .extend(val.split_whitespace().map(String::from)),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Ok(unit)
    }

    pub fn to_systemd_content(&self) -> String {
        let mut lines = Vec::new();
        lines.push("[Unit]".to_string());
        if let Some(desc) = &self.unit.description {
            lines.push(format!("Description={}", desc));
        }
        if !self.unit.after.is_empty() {
            lines.push(format!("After={}", self.unit.after.join(" ")));
        }
        if !self.unit.requires.is_empty() {
            lines.push(format!("Requires={}", self.unit.requires.join(" ")));
        }
        if !self.unit.wants.is_empty() {
            lines.push(format!("Wants={}", self.unit.wants.join(" ")));
        }
        if !self.unit.before.is_empty() {
            lines.push(format!("Before={}", self.unit.before.join(" ")));
        }
        lines.push(String::new());

        lines.push("[Service]".to_string());
        if !self.service.service_type.is_empty() {
            lines.push(format!("Type={}", self.service.service_type));
        }
        if let Some(user) = &self.service.user {
            lines.push(format!("User={}", user));
        }
        if let Some(group) = &self.service.group {
            lines.push(format!("Group={}", group));
        }
        if let Some(wd) = &self.service.working_directory {
            lines.push(format!("WorkingDirectory={}", wd.display()));
        }
        if let Some(exec_start) = &self.service.exec_start {
            lines.push(format!("ExecStart={}", exec_start));
        }
        if let Some(exec_stop) = &self.service.exec_stop {
            lines.push(format!("ExecStop={}", exec_stop));
        }
        if let Some(exec_reload) = &self.service.exec_reload {
            lines.push(format!("ExecReload={}", exec_reload));
        }
        if let Some(restart) = &self.service.restart {
            lines.push(format!("Restart={}", restart));
        }
        if let Some(sec) = self.service.restart_sec {
            lines.push(format!("RestartSec={}s", sec));
        }
        for (k, v) in &self.service.environments {
            lines.push(format!("Environment=\"{}={}\"", k, v));
        }
        for env_file in &self.service.environment_files {
            let prefix = if env_file.optional { "-" } else { "" };
            lines.push(format!("EnvironmentFile={}{}", prefix, env_file.path.display()));
        }
        if self.service.private_tmp {
            lines.push("PrivateTmp=true".to_string());
        }
        if let Some(pid_file) = &self.service.pid_file {
            lines.push(format!("PIDFile={}", pid_file.display()));
        }
        lines.push(String::new());

        lines.push("[Install]".to_string());
        if !self.install.wanted_by.is_empty() {
            lines.push(format!("WantedBy={}", self.install.wanted_by.join(" ")));
        }
        lines.push(String::new());

        lines.join("\n")
    }
}

fn parse_environment_line(val: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut chars = val.chars().peekable();
    let mut current_token = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    while let Some(&c) = chars.peek() {
        chars.next();
        if (c == '"' || c == '\'') && !in_quotes {
            in_quotes = true;
            quote_char = c;
        } else if in_quotes && c == quote_char {
            in_quotes = false;
        } else if c.is_whitespace() && !in_quotes {
            if !current_token.is_empty() {
                if let Some((k, v)) = parse_env_pair(&current_token) {
                    result.push((k, v));
                }
                current_token.clear();
            }
        } else {
            current_token.push(c);
        }
    }
    if !current_token.is_empty() {
        if let Some((k, v)) = parse_env_pair(&current_token) {
            result.push((k, v));
        }
    }
    result
}

fn parse_env_pair(s: &str) -> Option<(String, String)> {
    let s = s.trim_matches('"').trim_matches('\'');
    s.split_once('=')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sample_service() {
        let sample = r#"
[Unit]
Description=rclone
After=network.target

[Service]
Type=exec
User=root
Group=root
Restart=always
PrivateTmp=true
Environment="SECRET=abcdefg"
EnvironmentFile=/root/file.environment
ExecStart=rclone serve webdav --addr 0.0.0.0:8080 --read-only remote:/

[Install]
WantedBy=multi-user.target
"#;

        let unit = ServiceUnit::parse(sample).unwrap();
        assert_eq!(unit.unit.description.as_deref(), Some("rclone"));
        assert_eq!(unit.unit.after, vec!["network.target"]);
        assert_eq!(unit.service.service_type, "exec");
        assert_eq!(unit.service.user.as_deref(), Some("root"));
        assert_eq!(unit.service.group.as_deref(), Some("root"));
        assert_eq!(unit.service.restart.as_deref(), Some("always"));
        assert!(unit.service.private_tmp);
        assert_eq!(
            unit.service.environments,
            vec![("SECRET".to_string(), "abcdefg".to_string())]
        );
        assert_eq!(unit.service.environment_files.len(), 1);
        assert_eq!(
            unit.service.environment_files[0].path,
            PathBuf::from("/root/file.environment")
        );
        assert_eq!(
            unit.service.exec_start.as_deref(),
            Some("rclone serve webdav --addr 0.0.0.0:8080 --read-only remote:/")
        );
        assert_eq!(unit.install.wanted_by, vec!["multi-user.target"]);
    }
}
