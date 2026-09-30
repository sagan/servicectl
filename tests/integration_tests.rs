use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn get_binary_path() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove binary name
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("servicectl").to_string_lossy().to_string()
}

#[test]
fn test_systemd_add_and_replace() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("systemd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let service_file = temp_dir.path().join("foo.service");
    let content = r#"[Unit]
Description=Foo Service
After=network.target

[Service]
Type=exec
ExecStart=/usr/bin/foo --option
Restart=always

[Install]
WantedBy=multi-user.target
"#;
    fs::write(&service_file, content).unwrap();

    // 1. Add subcommand
    let output = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "systemd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stdout: {}, stderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));

    let installed_file = sys_dir.join("foo.service");
    assert!(installed_file.exists());
    assert_eq!(fs::read_to_string(&installed_file).unwrap(), content);

    // 2. Add subcommand again should fail
    let output2 = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "systemd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output2.status.success());
    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr.contains("already exists"), "stderr was: {}", stderr);

    // 3. Replace subcommand should succeed
    let updated_content = r#"[Unit]
Description=Foo Service Updated

[Service]
ExecStart=/usr/bin/foo --updated
"#;
    fs::write(&service_file, updated_content).unwrap();

    let output3 = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args([
            "replace",
            service_file.to_str().unwrap(),
            "--target",
            "systemd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output3.status.success(), "stdout: {}, stderr: {}", String::from_utf8_lossy(&output3.stdout), String::from_utf8_lossy(&output3.stderr));
    assert_eq!(fs::read_to_string(&installed_file).unwrap(), updated_content);
}

#[test]
fn test_openrc_add_and_replace() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("openrc_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let service_file = temp_dir.path().join("rclone.service");
    let content = r#"[Unit]
Description=rclone
After=network.target

[Service]
Type=exec
User=root
Restart=always
PrivateTmp=true
Environment="SECRET=abcdefg"
EnvironmentFile=/root/file.environment
ExecStart=rclone serve webdav --addr 0.0.0.0:8080 --read-only remote:/

[Install]
WantedBy=multi-user.target
"#;
    fs::write(&service_file, content).unwrap();

    // 1. Add subcommand
    let output = Command::new(&bin)
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stdout: {}, stderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));

    let installed_file = sys_dir.join("rclone");
    assert!(installed_file.exists());
    let perms = fs::metadata(&installed_file).unwrap().permissions();
    assert_ne!(perms.mode() & 0o111, 0, "file must be executable");

    let script = fs::read_to_string(&installed_file).unwrap();
    assert!(script.contains("#!/sbin/openrc-run"));
    assert!(script.contains("command=\"rclone\""));
    assert!(script.contains("command_args=\"serve webdav --addr 0.0.0.0:8080 --read-only remote:/\""));
    assert!(script.contains("command_user=\"root\""));
    assert!(script.contains("supervisor=\"supervise-daemon\""));
    assert!(script.contains("if [ -f \"/root/file.environment\" ]; then"));
    assert!(script.contains(". \"/root/file.environment\""));
    assert!(script.contains("export SECRET=\"abcdefg\""));
    assert!(script.contains("need net"));

    // 2. Add subcommand again should fail
    let output2 = Command::new(&bin)
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output2.status.success());
    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr.contains("already exists"));

    // 3. Replace subcommand should succeed
    let output3 = Command::new(&bin)
        .args([
            "replace",
            service_file.to_str().unwrap(),
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output3.status.success());
}

#[test]
fn test_procd_add_and_replace() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let service_file = temp_dir.path().join("webdav.service");
    let content = r#"[Unit]
Description=WebDAV Service
After=network.target

[Service]
Type=simple
User=nobody
Environment="PORT=8080"
EnvironmentFile=/etc/config/webdav.env
ExecStart=/usr/bin/webdav_server --port 8080
Restart=always
RestartSec=10s
"#;
    fs::write(&service_file, content).unwrap();

    // 1. Add subcommand
    let output = Command::new(&bin)
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stdout: {}, stderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));

    let installed_file = sys_dir.join("webdav");
    assert!(installed_file.exists());
    let perms = fs::metadata(&installed_file).unwrap().permissions();
    assert_ne!(perms.mode() & 0o111, 0, "file must be executable");

    let script = fs::read_to_string(&installed_file).unwrap();
    assert!(script.contains("#!/bin/sh /etc/rc.common"));
    assert!(script.contains("USE_PROCD=1"));
    assert!(script.contains("procd_open_instance \"webdav\""));
    assert!(script.contains("if [ -f \"/etc/config/webdav.env\" ]; then"));
    assert!(script.contains(". \"/etc/config/webdav.env\""));
    assert!(script.contains("export PORT=\"8080\""));
    assert!(script.contains("procd_append_param env PORT=\"8080\""));
    assert!(script.contains("procd_set_param command /usr/bin/webdav_server --port 8080"));
    assert!(script.contains("procd_set_param user \"nobody\""));
    assert!(script.contains("procd_set_param respawn 3600 10 0"));
    assert!(script.contains("procd_close_instance"));

    // 2. Add subcommand again should fail
    let output2 = Command::new(&bin)
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output2.status.success());
    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr.contains("already exists"));

    // 3. Replace subcommand should succeed
    let output3 = Command::new(&bin)
        .args([
            "replace",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output3.status.success());
}

#[test]
fn test_openrc_user_and_group_normal_and_busybox() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("openrc_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let service_file = temp_dir.path().join("usergroup.service");
    let content = r#"[Unit]
Description=User Group Test Service

[Service]
ExecStart=/usr/bin/test_app
User=myuser
Group=mygroup
"#;
    fs::write(&service_file, content).unwrap();

    // 1. Normal start-stop-daemon flags (-u, -g)
    let output_normal = Command::new(&bin)
        .env("SERVICECTL_SSD_TYPE", "normal")
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_normal.status.success());
    let script_normal = fs::read_to_string(sys_dir.join("usergroup")).unwrap();
    assert!(script_normal.contains("command_user=\"myuser:mygroup\""));
    assert!(script_normal.contains("start_stop_daemon_args=\"-u myuser -g mygroup\""));

    // 2. Busybox start-stop-daemon flags (-c USER:GRP)
    let output_busybox = Command::new(&bin)
        .env("SERVICECTL_SSD_TYPE", "busybox")
        .args([
            "replace",
            service_file.to_str().unwrap(),
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_busybox.status.success());
    let script_busybox = fs::read_to_string(sys_dir.join("usergroup")).unwrap();
    assert!(script_busybox.contains("command_user=\"myuser:mygroup\""));
    assert!(script_busybox.contains("start_stop_daemon_args=\"-c myuser:mygroup\""));
}

#[test]
fn test_procd_user_and_group() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let service_file = temp_dir.path().join("usergroup.service");
    let content = r#"[Unit]
Description=Procd User Group Test

[Service]
ExecStart=/usr/bin/test_app
User=myuser
Group=mygroup
"#;
    fs::write(&service_file, content).unwrap();

    let output = Command::new(&bin)
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let script = fs::read_to_string(sys_dir.join("usergroup")).unwrap();
    assert!(script.contains("procd_set_param user \"myuser\""));
    assert!(script.contains("procd_set_param group \"mygroup\""));
    // Since user is 'myuser' (not root/unset), default HOME/USER should not be inserted
    assert!(!script.contains("procd_append_param env HOME=\"/root\""));
}

#[test]
fn test_procd_numeric_user_and_group_translation() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let passwd_file = temp_dir.path().join("passwd");
    fs::write(
        &passwd_file,
        "root:x:0:0:root:/root:/bin/sh\ndummyuser:x:1001:2002:Dummy User:/home/dummyuser:/bin/sh\n",
    )
    .unwrap();

    let group_file = temp_dir.path().join("group");
    fs::write(&group_file, "root:x:0:\ndummygroup:x:2002:\n").unwrap();

    let service_file = temp_dir.path().join("numeric_ug.service");
    let content = r#"[Unit]
Description=Procd Numeric User Group Test

[Service]
ExecStart=/usr/bin/test_app
User=1001
Group=2002
"#;
    fs::write(&service_file, content).unwrap();

    let output = Command::new(&bin)
        .env("SERVICECTL_PASSWD_PATH", passwd_file.to_str().unwrap())
        .env("SERVICECTL_GROUP_PATH", group_file.to_str().unwrap())
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let script = fs::read_to_string(sys_dir.join("numeric_ug")).unwrap();
    assert!(script.contains("procd_set_param user \"dummyuser\""));
    assert!(script.contains("procd_set_param group \"dummygroup\""));
}

#[test]
fn test_procd_numeric_user_not_found_error() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let passwd_file = temp_dir.path().join("passwd");
    fs::write(&passwd_file, "root:x:0:0:root:/root:/bin/sh\n").unwrap();

    let service_file = temp_dir.path().join("missing_user.service");
    let content = r#"[Unit]
Description=Missing User Test

[Service]
ExecStart=/usr/bin/test_app
User=9999
"#;
    fs::write(&service_file, content).unwrap();

    let output = Command::new(&bin)
        .env("SERVICECTL_PASSWD_PATH", passwd_file.to_str().unwrap())
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("User with UID '9999' not found"));
}

#[test]
fn test_procd_numeric_group_not_found_error() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_dir = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir).unwrap();

    let group_file = temp_dir.path().join("group");
    fs::write(&group_file, "root:x:0:\n").unwrap();

    let service_file = temp_dir.path().join("missing_group.service");
    let content = r#"[Unit]
Description=Missing Group Test

[Service]
ExecStart=/usr/bin/test_app
Group=8888
"#;
    fs::write(&service_file, content).unwrap();

    let output = Command::new(&bin)
        .env("SERVICECTL_GROUP_PATH", group_file.to_str().unwrap())
        .args([
            "add",
            service_file.to_str().unwrap(),
            "--target",
            "procd",
            "--sys-dir",
            sys_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Group with GID '8888' not found"));
}

#[test]
fn test_adhoc_simple_command() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. Systemd target
    let sys_dir_systemd = temp_dir.path().join("systemd_sys");
    fs::create_dir_all(&sys_dir_systemd).unwrap();

    let output_sysd = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args([
            "add",
            "--adhoc",
            "rclone serve webdav /",
            "--target",
            "systemd",
            "--sys-dir",
            sys_dir_systemd.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_sysd.status.success(), "stderr: {}", String::from_utf8_lossy(&output_sysd.stderr));
    let sysd_file = sys_dir_systemd.join("rclone.service");
    assert!(sysd_file.exists());
    let sysd_content = fs::read_to_string(&sysd_file).unwrap();
    assert!(sysd_content.contains("Description=rclone"));
    assert!(sysd_content.contains("ExecStart="));
    assert!(sysd_content.contains("serve webdav /"));
    assert!(sysd_content.contains("Restart=always"));

    // 2. OpenRC target with --name override
    let sys_dir_openrc = temp_dir.path().join("openrc_sys");
    fs::create_dir_all(&sys_dir_openrc).unwrap();

    let output_openrc = Command::new(&bin)
        .args([
            "add",
            "--adhoc",
            "--name",
            "custom-rclone",
            "rclone serve webdav /",
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir_openrc.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_openrc.status.success(), "stderr: {}", String::from_utf8_lossy(&output_openrc.stderr));
    let openrc_file = sys_dir_openrc.join("custom-rclone");
    assert!(openrc_file.exists());
    let openrc_content = fs::read_to_string(&openrc_file).unwrap();
    assert!(openrc_content.contains("name=\"custom-rclone\""));
    assert!(openrc_content.contains("command="));

    // 3. Replace subcommand with adhoc
    let output_replace = Command::new(&bin)
        .args([
            "replace",
            "--adhoc",
            "--name",
            "custom-rclone",
            "rclone serve webdav / --read-only",
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir_openrc.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_replace.status.success(), "stderr: {}", String::from_utf8_lossy(&output_replace.stderr));
    let openrc_content_updated = fs::read_to_string(&openrc_file).unwrap();
    assert!(openrc_content_updated.contains("--read-only"));

    // 4. Procd target with adhoc
    let sys_dir_procd = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir_procd).unwrap();

    let output_procd = Command::new(&bin)
        .args([
            "add",
            "--adhoc",
            "rclone serve webdav /",
            "--target",
            "procd",
            "--sys-dir",
            sys_dir_procd.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_procd.status.success(), "stderr: {}", String::from_utf8_lossy(&output_procd.stderr));
    let procd_file = sys_dir_procd.join("rclone");
    assert!(procd_file.exists());
    let procd_content = fs::read_to_string(&procd_file).unwrap();
    assert!(procd_content.contains("procd_append_param env HOME=\"/root\""));
    assert!(procd_content.contains("procd_append_param env USER=\"root\""));
    assert!(procd_content.contains("procd_set_param respawn"));
}

#[test]
fn test_adhoc_start_stop_daemon_command() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();

    let sys_dir_openrc = temp_dir.path().join("openrc_sys");
    fs::create_dir_all(&sys_dir_openrc).unwrap();

    let ssd_cmd = "start-stop-daemon -S -c 0:65501 -p /var/run/rclone.pid -b -x rclone -- serve webdav /";

    let output = Command::new(&bin)
        .args([
            "add",
            "--adhoc",
            ssd_cmd,
            "--target",
            "openrc",
            "--sys-dir",
            sys_dir_openrc.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let openrc_file = sys_dir_openrc.join("rclone");
    assert!(openrc_file.exists());
    let script = fs::read_to_string(&openrc_file).unwrap();
    assert!(script.contains("command_user=\"0:65501\""));
    assert!(script.contains("pidfile=\"/var/run/rclone.pid\""));
    assert!(script.contains("command="));
    assert!(script.contains("command_args=\"serve webdav /\""));

    // Also test systemd target output for ssd adhoc
    let sys_dir_systemd = temp_dir.path().join("systemd_sys");
    fs::create_dir_all(&sys_dir_systemd).unwrap();

    let output_sysd = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args([
            "add",
            "--adhoc",
            ssd_cmd,
            "--target",
            "systemd",
            "--sys-dir",
            sys_dir_systemd.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output_sysd.status.success(), "stderr: {}", String::from_utf8_lossy(&output_sysd.stderr));
    let sysd_file = sys_dir_systemd.join("rclone.service");
    assert!(sysd_file.exists());
    let sysd_content = fs::read_to_string(&sysd_file).unwrap();
    assert!(sysd_content.contains("User=0"));
    assert!(sysd_content.contains("Group=65501"));
    assert!(sysd_content.contains("PIDFile=/var/run/rclone.pid"));
}

#[test]
fn test_replace_identical_does_not_update_disk() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();

    // Systemd test
    let sys_dir_systemd = temp_dir.path().join("systemd_sys");
    fs::create_dir_all(&sys_dir_systemd).unwrap();

    let service_file = temp_dir.path().join("demo.service");
    let content = "[Unit]\nDescription=Demo Service\n\n[Service]\nExecStart=/usr/bin/demo\n";
    fs::write(&service_file, content).unwrap();

    let out = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args(["add", service_file.to_str().unwrap(), "--target", "systemd", "--sys-dir", sys_dir_systemd.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    let installed_sysd = sys_dir_systemd.join("demo.service");
    let mtime_sysd_before = fs::metadata(&installed_sysd).unwrap().modified().unwrap();

    // Small delay to ensure any potential filesystem timestamp update would differ
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Replace with identical content
    let out_rep = Command::new(&bin)
        .env("SERVICECTL_SKIP_SYSTEMCTL", "1")
        .args(["replace", service_file.to_str().unwrap(), "--target", "systemd", "--sys-dir", sys_dir_systemd.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out_rep.status.success());
    let mtime_sysd_after = fs::metadata(&installed_sysd).unwrap().modified().unwrap();
    assert_eq!(mtime_sysd_before, mtime_sysd_after);

    // OpenRC test
    let sys_dir_openrc = temp_dir.path().join("openrc_sys");
    fs::create_dir_all(&sys_dir_openrc).unwrap();

    let out_openrc = Command::new(&bin)
        .args(["add", service_file.to_str().unwrap(), "--target", "openrc", "--sys-dir", sys_dir_openrc.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out_openrc.status.success());

    let installed_openrc = sys_dir_openrc.join("demo");
    let mtime_openrc_before = fs::metadata(&installed_openrc).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));

    let out_openrc_rep = Command::new(&bin)
        .args(["replace", service_file.to_str().unwrap(), "--target", "openrc", "--sys-dir", sys_dir_openrc.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out_openrc_rep.status.success());
    let mtime_openrc_after = fs::metadata(&installed_openrc).unwrap().modified().unwrap();
    assert_eq!(mtime_openrc_before, mtime_openrc_after);

    // Procd test
    let sys_dir_procd = temp_dir.path().join("procd_sys");
    fs::create_dir_all(&sys_dir_procd).unwrap();

    let out_procd = Command::new(&bin)
        .args(["add", service_file.to_str().unwrap(), "--target", "procd", "--sys-dir", sys_dir_procd.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out_procd.status.success());

    let installed_procd = sys_dir_procd.join("demo");
    let mtime_procd_before = fs::metadata(&installed_procd).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));

    let out_procd_rep = Command::new(&bin)
        .args(["replace", service_file.to_str().unwrap(), "--target", "procd", "--sys-dir", sys_dir_procd.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out_procd_rep.status.success());
    let mtime_procd_after = fs::metadata(&installed_procd).unwrap().modified().unwrap();
    assert_eq!(mtime_procd_before, mtime_procd_after);
}

#[test]
fn test_systemd_action_subcommands() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let log_file = temp_dir.path().join("systemctl.log");
    let mock_systemctl = bin_dir.join("systemctl");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\n",
        log_file.display()
    );
    fs::write(&mock_systemctl, script).unwrap();
    fs::set_permissions(&mock_systemctl, fs::Permissions::from_mode(0o755)).unwrap();

    let path_env = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap_or_default());

    let actions = ["enable", "disable", "start", "stop", "restart", "status"];
    for action in actions {
        let output = Command::new(&bin)
            .env("PATH", &path_env)
            .args(["--target", "systemd", action, "testsvc"])
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed on action {}: stderr: {}", action, String::from_utf8_lossy(&output.stderr));
    }

    let logs = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = logs.lines().collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0], "enable testsvc");
    assert_eq!(lines[1], "disable testsvc");
    assert_eq!(lines[2], "start testsvc");
    assert_eq!(lines[3], "stop testsvc");
    assert_eq!(lines[4], "restart testsvc");
    assert_eq!(lines[5], "status testsvc");
}

#[test]
fn test_procd_action_subcommands() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let log_file = temp_dir.path().join("service.log");
    let mock_service = bin_dir.join("service");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\n",
        log_file.display()
    );
    fs::write(&mock_service, script).unwrap();
    fs::set_permissions(&mock_service, fs::Permissions::from_mode(0o755)).unwrap();

    let path_env = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap_or_default());

    let actions = ["enable", "disable", "start", "stop", "restart", "status"];
    for action in actions {
        let output = Command::new(&bin)
            .env("PATH", &path_env)
            .args(["--target", "procd", action, "mysvc.service"])
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed on action {}: stderr: {}", action, String::from_utf8_lossy(&output.stderr));
    }

    let logs = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = logs.lines().collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0], "mysvc enable");
    assert_eq!(lines[1], "mysvc disable");
    assert_eq!(lines[2], "mysvc start");
    assert_eq!(lines[3], "mysvc stop");
    assert_eq!(lines[4], "mysvc restart");
    assert_eq!(lines[5], "mysvc status");
}

#[test]
fn test_openrc_action_subcommands() {
    let bin = get_binary_path();
    let temp_dir = tempfile::tempdir().unwrap();
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let update_log = temp_dir.path().join("rc_update.log");
    let mock_rc_update = bin_dir.join("rc-update");
    let update_script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\n",
        update_log.display()
    );
    fs::write(&mock_rc_update, update_script).unwrap();
    fs::set_permissions(&mock_rc_update, fs::Permissions::from_mode(0o755)).unwrap();

    let service_log = temp_dir.path().join("rc_service.log");
    let mock_rc_service = bin_dir.join("rc-service");
    let service_script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\n",
        service_log.display()
    );
    fs::write(&mock_rc_service, service_script).unwrap();
    fs::set_permissions(&mock_rc_service, fs::Permissions::from_mode(0o755)).unwrap();

    let path_env = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap_or_default());

    // 1. Enable & Disable
    let out_enable = Command::new(&bin)
        .env("PATH", &path_env)
        .args(["--target", "openrc", "enable", "app.service"])
        .output()
        .unwrap();
    assert!(out_enable.status.success());

    let out_disable = Command::new(&bin)
        .env("PATH", &path_env)
        .args(["--target", "openrc", "disable", "app.service"])
        .output()
        .unwrap();
    assert!(out_disable.status.success());

    let update_content = fs::read_to_string(&update_log).unwrap();
    let update_lines: Vec<&str> = update_content.lines().collect();
    assert_eq!(update_lines, vec!["add app default", "del app default"]);

    // 2. Start, Stop, Restart, Status
    for action in ["start", "stop", "restart", "status"] {
        let out = Command::new(&bin)
            .env("PATH", &path_env)
            .args(["--target", "openrc", action, "app.service"])
            .output()
            .unwrap();
        assert!(out.status.success());
    }

    let service_content = fs::read_to_string(&service_log).unwrap();
    let service_lines: Vec<&str> = service_content.lines().collect();
    assert_eq!(service_lines, vec!["app start", "app stop", "app restart", "app status"]);
}

#[test]
fn test_dry_run_output() {
    let bin = get_binary_path();

    let out = Command::new(&bin)
        .env("SERVICECTL_DRY_RUN", "1")
        .args(["--target", "systemd", "status", "nginx"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "systemctl status nginx");

    let out = Command::new(&bin)
        .env("SERVICECTL_DRY_RUN", "1")
        .args(["--target", "procd", "status", "nginx"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "service nginx status");

    let out = Command::new(&bin)
        .env("SERVICECTL_DRY_RUN", "1")
        .args(["--target", "openrc", "enable", "nginx"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "rc-update add nginx default");
}


