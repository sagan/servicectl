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


