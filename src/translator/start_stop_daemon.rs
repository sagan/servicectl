use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStopDaemonKind {
    Normal,  // Standard / Alpine / dpkg version (-u user -g group)
    Busybox, // Busybox version (-c USER[:GRP])
}

impl StartStopDaemonKind {
    pub fn detect() -> Self {
        // 1. Check environment variable override
        if let Ok(env_val) = std::env::var("SERVICECTL_SSD_TYPE")
            .or_else(|_| std::env::var("SERVICECTL_START_STOP_DAEMON_TYPE"))
        {
            let lower = env_val.to_lowercase();
            if lower == "busybox" || lower == "true" || lower == "1" {
                return Self::Busybox;
            } else if lower == "normal" || lower == "alpine" || lower == "dpkg" || lower == "standard" {
                return Self::Normal;
            }
        }

        // 2. Check if binary path links to busybox
        let ssd_paths = [
            "/sbin/start-stop-daemon",
            "/usr/sbin/start-stop-daemon",
            "/bin/start-stop-daemon",
        ];
        for path in &ssd_paths {
            if let Ok(target) = std::fs::read_link(path) {
                if target.to_string_lossy().contains("busybox") {
                    return Self::Busybox;
                }
            }
        }

        // 3. Try running start-stop-daemon --help or --version
        if let Ok(output) = Command::new("start-stop-daemon").arg("--help").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);
            if combined.contains("BusyBox") || combined.contains("busybox") {
                return Self::Busybox;
            }
            if combined.contains("-u, --user") || combined.contains("--group") {
                return Self::Normal;
            }
        }

        if let Ok(output) = Command::new("start-stop-daemon").arg("--version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);
            if combined.contains("BusyBox") || combined.contains("busybox") {
                return Self::Busybox;
            }
        }

        // 4. Fallback: check if busybox binary exists and start-stop-daemon does not exist standalone
        if (Path::new("/bin/busybox").exists() || Path::new("/sbin/busybox").exists())
            && !Path::new("/sbin/openrc-run").exists()
            && !Path::new("/usr/bin/dpkg").exists()
        {
            return Self::Busybox;
        }

        Self::Normal
    }

    pub fn build_flags(&self, user: Option<&str>, group: Option<&str>) -> Vec<String> {
        let mut flags = Vec::new();
        match self {
            Self::Normal => {
                if let Some(u) = user {
                    flags.push("-u".to_string());
                    flags.push(u.to_string());
                }
                if let Some(g) = group {
                    flags.push("-g".to_string());
                    flags.push(g.to_string());
                }
            }
            Self::Busybox => match (user, group) {
                (Some(u), Some(g)) => {
                    flags.push("-c".to_string());
                    flags.push(format!("{}:{}", u, g));
                }
                (Some(u), None) => {
                    flags.push("-c".to_string());
                    flags.push(u.to_string());
                }
                (None, Some(g)) => {
                    flags.push("-c".to_string());
                    flags.push(format!(":{}", g));
                }
                (None, None) => {}
            },
        }
        flags
    }

    pub fn build_args_string(&self, user: Option<&str>, group: Option<&str>) -> Option<String> {
        let flags = self.build_flags(user, group);
        if flags.is_empty() {
            None
        } else {
            Some(flags.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_flags() {
        let ssd = StartStopDaemonKind::Normal;

        assert_eq!(
            ssd.build_flags(Some("myuser"), Some("mygroup")),
            vec!["-u", "myuser", "-g", "mygroup"]
        );
        assert_eq!(
            ssd.build_args_string(Some("myuser"), Some("mygroup")),
            Some("-u myuser -g mygroup".to_string())
        );

        assert_eq!(
            ssd.build_flags(Some("myuser"), None),
            vec!["-u", "myuser"]
        );
        assert_eq!(
            ssd.build_args_string(Some("myuser"), None),
            Some("-u myuser".to_string())
        );

        assert_eq!(
            ssd.build_flags(None, Some("mygroup")),
            vec!["-g", "mygroup"]
        );
        assert_eq!(
            ssd.build_args_string(None, Some("mygroup")),
            Some("-g mygroup".to_string())
        );

        assert_eq!(ssd.build_flags(None, None), Vec::<String>::new());
        assert_eq!(ssd.build_args_string(None, None), None);
    }

    #[test]
    fn test_busybox_flags() {
        let ssd = StartStopDaemonKind::Busybox;

        assert_eq!(
            ssd.build_flags(Some("myuser"), Some("mygroup")),
            vec!["-c", "myuser:mygroup"]
        );
        assert_eq!(
            ssd.build_args_string(Some("myuser"), Some("mygroup")),
            Some("-c myuser:mygroup".to_string())
        );

        assert_eq!(
            ssd.build_flags(Some("myuser"), None),
            vec!["-c", "myuser"]
        );
        assert_eq!(
            ssd.build_args_string(Some("myuser"), None),
            Some("-c myuser".to_string())
        );

        assert_eq!(
            ssd.build_flags(None, Some("mygroup")),
            vec!["-c", ":mygroup"]
        );
        assert_eq!(
            ssd.build_args_string(None, Some("mygroup")),
            Some("-c :mygroup".to_string())
        );

        assert_eq!(ssd.build_flags(None, None), Vec::<String>::new());
        assert_eq!(ssd.build_args_string(None, None), None);
    }
}
