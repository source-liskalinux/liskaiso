use std::env;
use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::io::BufRead;
use std::io::BufReader;
use std::sync::mpsc;
use std::thread;

const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[92m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

const EMBED_ZSHRC: &str = include_str!("./.zshrc");
const EMBED_OS_RELEASE: &str = include_str!("./os-release");

fn print_info(msg: &str) { println!("{}[i]{} {}", CYAN, RESET, msg); }
fn print_success(msg: &str) { println!("{}[+]{} {}{}{}", CYAN, RESET, GREEN, msg, RESET); }
fn print_error(msg: &str) { println!("{}[-]{} {}{}{}", CYAN, RESET, RED, msg, RESET); }

fn run_command(cmd: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|err| format!("Could not start {}: {}", cmd, err))?;
    if status.success() { 
        Ok(()) 
    } else { 
        Err(format!("Command {} failed with status: {:?}", cmd, status)) 
    }
}

struct Edition {
    id: &'static str,
    title: &'static str,
    packages: &'static [&'static str],
}

fn run_lkpm_smart_timeout(cmd: &str, args: &[&str]) -> Result<(), String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    let mut deadline: Option<std::time::Instant> = Some(std::time::Instant::now() + Duration::from_secs(30));
    let mut operation_started = false;
    #[derive(Debug)]
    enum ReaderMsg {
        OperationStarted,
    }
    let (tx, rx) = mpsc::channel::<ReaderMsg>();
    let tx_clone = tx.clone();
    let tx_clone_err = tx.clone();
    thread::spawn(move || {
        let tx = tx_clone;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let strip_ansi = |s: &str| -> String {
            let mut out = String::with_capacity(s.len());
            let mut chars = s.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    if let Some('[') = chars.peek() {
                        chars.next();
                        while let Some(nc) = chars.next() {
                            if ('@'..='~').contains(&nc) { break; }
                        }
                        continue;
                    }
                }
                out.push(c);
            }
            out
        };
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let raw = line.trim_end();
                    let l = strip_ansi(raw).replace('\r', "");
                    if l.contains("Initialize the operation") {
                        let _ = tx.send(ReaderMsg::OperationStarted);
                    }
                    println!("{}", l);
                }
                Err(_) => break,
            }
        }
    });
    thread::spawn(move || {
        let tx = tx_clone_err;
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        let strip_ansi = |s: &str| -> String {
            let mut out = String::with_capacity(s.len());
            let mut chars = s.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    if let Some('[') = chars.peek() {
                        chars.next();
                        while let Some(nc) = chars.next() {
                            if ('@'..='~').contains(&nc) { break; }
                        }
                        continue;
                    }
                }
                out.push(c);
            }
            out
        };
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let raw = line.trim_end();
                    let l = strip_ansi(raw).replace('\r', "");
                    if l.contains("Initialize the operation") {
                        let _ = tx.send(ReaderMsg::OperationStarted);
                    }
                    eprintln!("{}", l);
                }
                Err(_) => break,
            }
        }
    });
    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ReaderMsg::OperationStarted => {
                    if !operation_started {
                        operation_started = true;
                        deadline = None;
                        print_info("Timeout was disabled");
                    }
                }
            }
        }  
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                } else {
                    return Err("Command failed".to_string());
                }
            }
            Ok(None) => {
                if let Some(d) = deadline {
                    if std::time::Instant::now() > d {
                        let pid = child.id() as i32;
                        unsafe { libc::kill(-pid, libc::SIGKILL); }
                        child.kill().ok();
                        return Err("Timeout (30s) reached waiting for operation to start".to_string());
                    }
                }
            }
            Err(e) => return Err(e.to_string()),
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn install_package_pool(root: &Path, packages: &[&str]) -> Result<(), String> {
    let mut queue: Vec<String> = packages.iter().map(|s| s.to_string()).collect();
    let _ = run_command("lkpm", &["-r"]);
    while let Some(pkg) = queue.pop() {
        if pkg.ends_with(".so") {
            print_info(&format!("Skipping invalid package format: {}", pkg));
            continue;
        }
        print_info(&format!("Initializing: {}", pkg));
        if let Err(e) = run_lkpm_smart_timeout("lkpm", &["-i", "--root", root.to_str().unwrap(), "--noconfirm", &pkg]) {
            if pkg == "linux" {
                return Err(format!("CRITICAL: Failed to download linux kernel (error: {}). Build canceled!", e));
            }
            print_error(&format!("Skipping non-critical package {}: {}", pkg, e));
            continue;
        }
        let check = Command::new("lkpm").args(&["-l", &pkg, "--root", root.to_str().unwrap()]).output();
        if let Ok(out) = check {
            let report = String::from_utf8_lossy(&out.stdout);
            for line in report.lines() {
                if line.contains("(missing)") {
                    let dep = line.replace(">", "").replace("(missing)", "").trim().split_whitespace().next().unwrap_or("").to_string();
                    if !dep.is_empty() && !dep.ends_with(".so") && !queue.contains(&dep) {
                        queue.push(dep);
                    }
                }
            }
        }
    }
    let kernel_check = root.join("boot/vmlinuz-linux");
    if !kernel_check.exists() {
        return Err("FATAL: vmlinuz-linux not found on rootfs!".to_string());
    }
    Ok(())
}

const CLI_EDITION: Edition = Edition {
    id: "cli",
    title: "Liska Linux X86_64",
    packages: &[
        "linux", "linux-headers", "linux-firmware", "lkpm", "lkchroot", "lkstrap",
        "systemd", "dbus", "glibc", "busybox", "zsh", "sudo", "efibootmgr",
        "networkmanager", "modemmanager", "usb_modeswitch", "inetutils", "bash",
        "nano", "vim", "grub", "libverto", "wget", "curl", "git", "which", "man-db",
        "man-pages", "lkinit", "util-linux", "coreutils", "findutils", "sed", "grep",
        "kmod", "e2fsprogs", "iputils", "gptfdisk", "parted", "dosfstools", "btrfs-progs",
        "xfsprogs",
    ],
};

fn build_edition(edition: &Edition, workspace: &Path) -> Result<PathBuf, String> {
    let edition_root = workspace.join(format!("airootfs-{}", edition.id));
    let edition_iso_root = workspace.join(format!("iso_root-{}", edition.id));
    fs::remove_dir_all(&edition_root).ok();
    fs::remove_dir_all(&edition_iso_root).ok();
    fs::create_dir_all(&edition_root).ok();
    fs::create_dir_all(&edition_iso_root.join("boot")).ok();
    print_info(&format!("Building {} edition...", edition.title));
    install_package_pool(&edition_root, edition.packages)?;
    let sbin_dir = edition_root.join("sbin");
    let usr_sbin_dir = edition_root.join("usr/sbin");
    fs::create_dir_all(&sbin_dir).ok();
    fs::create_dir_all(&usr_sbin_dir).ok();
    let systemctl_target = "/usr/bin/systemctl";
    for cmd in &["reboot", "shutdown", "poweroff", "halt"] {
        let sbin_link = sbin_dir.join(cmd);
        let usr_sbin_link = usr_sbin_dir.join(cmd);
        let _ = fs::remove_file(&sbin_link);
        let _ = fs::remove_file(&usr_sbin_link);
        let _ = std::os::unix::fs::symlink(systemctl_target, &sbin_link);
        let _ = std::os::unix::fs::symlink(systemctl_target, &usr_sbin_link);
    }
    print_info("Enabling systemd core services....");
    let multi_user_wants = edition_root.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants).ok();
    let services_to_enable = &[
        ("NetworkManager.service", "/usr/lib/systemd/system/NetworkManager.service"),
        ("dbus.service", "/usr/lib/systemd/system/dbus.service"),
        ("seatd.service", "/usr/lib/systemd/system/seatd.service"),
        ("systemd-networkd.service", "/usr/lib/systemd/system/systemd-networkd.service"),
        ("systemd-resolved.service", "/usr/lib/systemd/system/systemd-resolved.service"),
    ];
    for (service_name, target_path) in services_to_enable {
        let symlink_path = multi_user_wants.join(service_name);
        if !symlink_path.exists() && edition_root.join(target_path.trim_start_matches('/')).exists() {
            let _ = std::os::unix::fs::symlink(target_path, &symlink_path);
        }
    }
    let global_zsh_dir = edition_root.join("etc/zsh");
    fs::create_dir_all(&global_zsh_dir).ok();
    let custom_zshrc_content = "
    export TERM=xterm-256color
    export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    ";
    fs::write(global_zsh_dir.join("zshrc"), custom_zshrc_content).map_err(|e| e.to_string())?;
    let candidates = [
        "src/.zshrc".to_string(),
    ];
    let root_zshrc = edition_root.join("root/.zshrc");
    fs::create_dir_all(edition_root.join("root")).ok();
    let mut placed = false;
    for cand in &candidates {
        let p = PathBuf::from(cand);
        if p.exists() {
            fs::copy(&p, &root_zshrc).map_err(|e| e.to_string())?;
            print_info(&format!("Integrated custom {} to /root/.zshrc....", p.display()));
            placed = true;
            break;
        }
    }
    if !placed {
        fs::write(&root_zshrc, EMBED_ZSHRC).map_err(|e| e.to_string())?;
        print_info(&format!("Installed embedded .zshrc for {} to /root/.zshrc....", edition.id));
    }
    let passwd_path = edition_root.join("etc/passwd");
    if passwd_path.exists() {
        if let Ok(content) = fs::read_to_string(&passwd_path) {
            let mut new_lines = Vec::new();
            for line in content.lines() {
                if line.starts_with("root:") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 7 {
                        new_lines.push(format!("{}:{}:{}:{}:{}:{}:/usr/bin/zsh", 
                            parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]));
                        continue;
                    }
                }
                new_lines.push(line.to_string());
            }
            let _ = fs::write(&passwd_path, new_lines.join("\n"));
        }
    }
    let profile_path = edition_root.join("etc/profile");
    let auto_zsh_script = 
        "if [ -t 1 ] && [ -n \"$PS1\" ] && [ -x /usr/bin/zsh ] && [ \"$(basename \"$SHELL\")\" = \"bash\" ]; then\n\
             export SHELL=/usr/bin/zsh\n\
             export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\n\
             exec /usr/bin/zsh -l\n\
         fi\n\
        ";
    if let Ok(mut current_profile) = fs::read_to_string(&profile_path) {
        current_profile.push_str(auto_zsh_script);
        let _ = fs::write(&profile_path, current_profile);
    } else {
        let _ = fs::write(&profile_path, auto_zsh_script);
    }
    print_info("Injected zsh to /etc/profile.");
    let system_units_dir = edition_root.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&system_units_dir).ok();
    let target_getty = edition_root.join("lib/systemd/system/getty@.service");
    let link_getty = system_units_dir.join("getty@tty1.service");
    if target_getty.exists() && !link_getty.exists() {
        let _ = std::os::unix::fs::symlink("../getty@.service", &link_getty);
    }
    print_info("Configuring autologin for tty1....");
    let getty_override_dir = edition_root.join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&getty_override_dir).ok();
    let autologin_conf = 
        "[Service]\n\
         ExecStart=\n\
         ExecStart=-/sbin/agetty --autologin root --noclear %I $TERM\n\
        ";
    fs::write(getty_override_dir.join("override.conf"), autologin_conf)
        .map_err(|e| e.to_string())?;
    print_info("Setting default systemd timezone to UTC....");
    let localtime_path = edition_root.join("etc/localtime");
    let _ = fs::remove_file(&localtime_path);
    let _ = std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime_path);
    let _ = fs::write(edition_root.join("etc/timezone"), "UTC\n");
    let os_release_src = PathBuf::from("src/os-release");
    fs::create_dir_all(edition_root.join("etc")).ok();
    if os_release_src.exists() {
        fs::copy(&os_release_src, edition_root.join("etc/os-release")).map_err(|e| e.to_string())?;
        print_info(&format!("Copied os-release for {} from src/os-release.", edition.title));
    } else {
        fs::write(edition_root.join("etc/os-release"), EMBED_OS_RELEASE).map_err(|e| e.to_string())?;
        print_info(&format!("Installed embedded os-release for {}.", edition.title));
    }
    let kernel_src = edition_root.join("boot/vmlinuz-linux");
    if !kernel_src.exists() {
        return Err("FATAL: vmlinuz-linux not found in rootfs!".into());
    }
    fs::copy(&kernel_src, edition_iso_root.join("boot/vmlinuz-linux")).map_err(|e| e.to_string())?;
    let sysctl_dir = edition_root.join("etc/sysctl.d");
    fs::create_dir_all(&sysctl_dir).ok();
    let _ = fs::write(
        sysctl_dir.join("20-quiet-printk.conf"),
        "kernel.printk = 3 3 3 3\n"
    );
    generate_pure_initramfs(&edition_root, &edition_iso_root)?;
    let squash_target = edition_iso_root.join("liskafs.sfs");
    print_info("Compressing filesystem into liskafs.sfs....");
    run_command("mksquashfs", &[
        edition_root.to_str().unwrap(),
        squash_target.to_str().unwrap(),
        "-comp", "zstd",
        "-noappend",
    ])?;
    write_grub_cfg(&edition_iso_root.join("boot/grub/grub.cfg"), edition.title)?;
    let iso_path = workspace.join(format!("liskalinux-{}-x86_64.iso", edition.id));
    print_info(&format!("Building ISO: {}", iso_path.display()));
    run_command("grub-mkrescue", &["-o", iso_path.to_str().unwrap(), edition_iso_root.to_str().unwrap()])?;
    print_success(&format!("Successfully built {} at {}.", edition.title, iso_path.display()));
    Ok(iso_path)
}

fn generate_pure_initramfs(rootfs: &Path, iso_root: &Path) -> Result<(), String> {
    print_info("Calling lkinit to generate initramfs...");
    let target_img = iso_root.join("boot/initramfs-liska.img");
    run_command("lkinit", &[
        "--root", rootfs.to_str().unwrap(),
        "--output", target_img.to_str().unwrap(),
        "--iso"
    ])?;
    print_success("Liska Linux initramfs generated successfully!");
    Ok(())
}

fn write_grub_cfg(path: &Path, title: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let content = format!("
        set timeout=5\n\
        set default=0\n\
        menuentry \"{}\" {{\n\
            linux /boot/vmlinuz-linux rw console=tty1 loglevel=3 audit=0 systemd.show_status=1 quiet cow_spacesize=2G\n\
            initrd /boot/initramfs-liska.img\n\
        }}\n",
        title
    );
    fs::write(path, content).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--help") {
        println!("");
        println!("-----------------------------------------------");
        println!("::: [ Liska Linux ISO Builder (v-1.0.0-1) ] :::");
        println!("-----------------------------------------------");
        println!("");
        println!("Usage: liskaiso <command>");
        println!("> --cli                    build CLI edition");
        println!("> --help                   display this help message");
        println!("");
        return;
    }
    if unsafe { libc::geteuid() } != 0 {
        print_error("Must be executed with root privileges.");
        exit(1);
    }
    let workspace = PathBuf::from("/home/janorovic/liskaiso-workspace");
    fs::create_dir_all(&workspace).ok();
    let edition = if args.len() > 1 && args[1] == "--cli" {
        &CLI_EDITION
    } else {
        print_error("No valid edition specified. Use --help for usage.");
        exit(1);
    };
    if let Err(e) = build_edition(edition, &workspace) {
        print_error(&format!("Failed to build {}: {}", edition.id, e));
        exit(1);
    }
    print_success("CLI edition has been built successfully!");
}
