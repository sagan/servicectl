use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::service::ServiceUnit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdhocResult {
    pub service_name: String,
    pub unit: ServiceUnit,
}

pub fn parse_adhoc_command(cmdline: &str, name_override: Option<&str>) -> Result<AdhocResult> {
    let tokens = shlex_split(cmdline)?;
    if tokens.is_empty() {
        bail!("Command line cannot be empty");
    }

    let mut user: Option<String> = None;
    let mut group: Option<String> = None;
    let mut pid_file: Option<PathBuf> = None;
    let mut work_dir: Option<PathBuf> = None;

    let (prog, args) = if is_start_stop_daemon(&tokens[0]) {
        parse_start_stop_daemon(&tokens, &mut user, &mut group, &mut pid_file, &mut work_dir)?
    } else {
        let prog = tokens[0].clone();
        let args = tokens[1..].to_vec();
        (prog, args)
    };

    let resolved_prog = resolve_binary_path(&prog);
    let inferred_name = infer_basename(&resolved_prog)?;
    let service_name = name_override
        .map(String::from)
        .unwrap_or(inferred_name);

    let exec_start = format_exec_start(&resolved_prog, &args);

    let mut unit = ServiceUnit::default();
    unit.unit.description = Some(service_name.clone());
    unit.service.service_type = "simple".to_string();
    unit.service.exec_start = Some(exec_start);
    unit.service.restart = Some("always".to_string());
    unit.service.user = user;
    unit.service.group = group;
    unit.service.pid_file = pid_file;
    unit.service.working_directory = work_dir;
    unit.install.wanted_by = vec!["multi-user.target".to_string()];

    Ok(AdhocResult {
        service_name,
        unit,
    })
}

pub fn shlex_split(s: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut has_token = false;

    for c in s.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            has_token = true;
            continue;
        }

        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
            has_token = true;
            continue;
        }

        if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                escaped = true;
            } else {
                current.push(c);
            }
            has_token = true;
            continue;
        }

        match c {
            '\\' => {
                escaped = true;
                has_token = true;
            }
            '\'' => {
                in_single = true;
                has_token = true;
            }
            '"' => {
                in_double = true;
                has_token = true;
            }
            c if c.is_whitespace() => {
                if has_token {
                    words.push(current.clone());
                    current.clear();
                    has_token = false;
                }
            }
            _ => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if escaped || in_single || in_double {
        bail!("Unterminated quote or trailing backslash in command line");
    }

    if has_token {
        words.push(current);
    }

    Ok(words)
}

fn is_start_stop_daemon(prog: &str) -> bool {
    let path = Path::new(prog);
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name == "start-stop-daemon")
        .unwrap_or(false)
}

fn parse_start_stop_daemon(
    tokens: &[String],
    user: &mut Option<String>,
    group: &mut Option<String>,
    pid_file: &mut Option<PathBuf>,
    work_dir: &mut Option<PathBuf>,
) -> Result<(String, Vec<String>)> {
    let double_dash_idx = tokens.iter().position(|t| t == "--");
    let end = double_dash_idx.unwrap_or(tokens.len());

    let mut exec_binary: Option<String> = None;
    let mut startas_binary: Option<String> = None;

    let mut i = 1;
    while i < end {
        let token = &tokens[i];

        let (key, val_inline) = if let Some((k, v)) = token.split_once('=') {
            (k, Some(v.to_string()))
        } else {
            (token.as_str(), None)
        };

        let get_val = |i_ref: &mut usize, inline: Option<String>, tokens: &[String]| -> Option<String> {
            if let Some(v) = inline {
                Some(v)
            } else if *i_ref + 1 < end {
                *i_ref += 1;
                Some(tokens[*i_ref].clone())
            } else {
                None
            }
        };

        match key {
            "-c" | "--chuid" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    if let Some((u, g)) = val.split_once(':') {
                        if !u.is_empty() {
                            *user = Some(u.to_string());
                        }
                        if !g.is_empty() {
                            *group = Some(g.to_string());
                        }
                    } else {
                        *user = Some(val);
                    }
                }
            }
            "-u" | "--user" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    *user = Some(val);
                }
            }
            "-g" | "--group" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    *group = Some(val);
                }
            }
            "-p" | "--pidfile" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    *pid_file = Some(PathBuf::from(val));
                }
            }
            "-d" | "--chdir" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    *work_dir = Some(PathBuf::from(val));
                }
            }
            "-x" | "--exec" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    exec_binary = Some(val);
                }
            }
            "-a" | "--startas" => {
                if let Some(val) = get_val(&mut i, val_inline, tokens) {
                    startas_binary = Some(val);
                }
            }
            "-n" | "--name" | "-s" | "--signal" | "-R" | "--retry" | "-k" | "--umask"
            | "-N" | "--nicelevel" | "-I" | "--procnodes" => {
                let _ = get_val(&mut i, val_inline, tokens);
            }
            "-S" | "--start" | "-K" | "--stop" | "-b" | "--background" | "-m" | "--make-pidfile"
            | "-o" | "--oknodo" | "-q" | "--quiet" | "-v" | "--verbose" | "-t" | "--test" => {}
            _ => {
                if key.starts_with("-c") && key.len() > 2 {
                    let val = &key[2..];
                    if let Some((u, g)) = val.split_once(':') {
                        if !u.is_empty() {
                            *user = Some(u.to_string());
                        }
                        if !g.is_empty() {
                            *group = Some(g.to_string());
                        }
                    } else {
                        *user = Some(val.to_string());
                    }
                } else if key.starts_with("-u") && key.len() > 2 {
                    *user = Some(key[2..].to_string());
                } else if key.starts_with("-g") && key.len() > 2 {
                    *group = Some(key[2..].to_string());
                } else if key.starts_with("-p") && key.len() > 2 {
                    *pid_file = Some(PathBuf::from(&key[2..]));
                } else if key.starts_with("-x") && key.len() > 2 {
                    exec_binary = Some(key[2..].to_string());
                } else if key.starts_with("-a") && key.len() > 2 {
                    startas_binary = Some(key[2..].to_string());
                }
            }
        }
        i += 1;
    }

    let specified_binary = startas_binary.or(exec_binary);
    let after_dash = if let Some(idx) = double_dash_idx {
        &tokens[idx + 1..]
    } else {
        &[]
    };

    let (prog, args) = match specified_binary {
        Some(bin) => {
            if !after_dash.is_empty() {
                let first_after = &after_dash[0];
                let bin_name = Path::new(&bin).file_name();
                let after_name = Path::new(first_after).file_name();
                if first_after == &bin || (bin_name.is_some() && bin_name == after_name) {
                    (bin, after_dash[1..].to_vec())
                } else {
                    (bin, after_dash.to_vec())
                }
            } else {
                (bin, Vec::new())
            }
        }
        None => {
            if !after_dash.is_empty() {
                (after_dash[0].clone(), after_dash[1..].to_vec())
            } else {
                bail!("Could not determine target executable from start-stop-daemon command line");
            }
        }
    };

    Ok((prog, args))
}

fn resolve_binary_path(prog: &str) -> String {
    let path = Path::new(prog);
    if path.is_absolute() {
        return prog.to_string();
    }

    if let Some(path_env) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join(prog);
            if candidate.is_file() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    prog.to_string()
}

fn infer_basename(prog: &str) -> Result<String> {
    let name = Path::new(prog)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Could not infer service name from program path '{}'", prog))?;
    Ok(name)
}

fn format_exec_start(prog: &str, args: &[String]) -> String {
    if args.is_empty() {
        return prog.to_string();
    }

    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(format_token(prog));
    for arg in args {
        parts.push(format_token(arg));
    }
    parts.join(" ")
}

fn format_token(token: &str) -> String {
    if token.is_empty() {
        return "\"\"".to_string();
    }
    if token.contains(' ') || token.contains('\t') || token.contains('"') || token.contains('\'') {
        if !token.contains('"') {
            format!("\"{}\"", token)
        } else if !token.contains('\'') {
            format!("'{}'", token)
        } else {
            format!("\"{}\"", token.replace('"', "\\\""))
        }
    } else {
        token.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shlex_split_basic() {
        let res = shlex_split("rclone serve webdav /").unwrap();
        assert_eq!(res, vec!["rclone", "serve", "webdav", "/"]);
    }

    #[test]
    fn test_shlex_split_quotes() {
        let res = shlex_split("cmd --title \"My App\" 'single quoted'").unwrap();
        assert_eq!(res, vec!["cmd", "--title", "My App", "single quoted"]);
    }

    #[test]
    fn test_parse_simple_adhoc() {
        let res = parse_adhoc_command("rclone serve webdav /", None).unwrap();
        assert_eq!(res.service_name, "rclone");
        assert_eq!(res.unit.unit.description.as_deref(), Some("rclone"));
        assert!(res.unit.service.exec_start.as_ref().unwrap().ends_with("rclone serve webdav /"));
        assert_eq!(res.unit.service.restart.as_deref(), Some("always"));
        assert_eq!(res.unit.service.user, None);
        assert_eq!(res.unit.service.group, None);
        assert_eq!(res.unit.service.pid_file, None);
    }

    #[test]
    fn test_parse_adhoc_with_name_override() {
        let res = parse_adhoc_command("rclone serve webdav /", Some("my-webdav")).unwrap();
        assert_eq!(res.service_name, "my-webdav");
        assert_eq!(res.unit.unit.description.as_deref(), Some("my-webdav"));
    }

    #[test]
    fn test_parse_start_stop_daemon_adhoc() {
        let cmd = "start-stop-daemon -S -c 0:65501 -p /var/run/rclone.pid -b -x rclone -- serve webdav /";
        let res = parse_adhoc_command(cmd, None).unwrap();
        assert_eq!(res.service_name, "rclone");
        assert_eq!(res.unit.service.user.as_deref(), Some("0"));
        assert_eq!(res.unit.service.group.as_deref(), Some("65501"));
        assert_eq!(
            res.unit.service.pid_file,
            Some(PathBuf::from("/var/run/rclone.pid"))
        );
        assert!(res.unit.service.exec_start.as_ref().unwrap().ends_with("rclone serve webdav /"));
    }
}
