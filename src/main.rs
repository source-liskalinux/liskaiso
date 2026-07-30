use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[92m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

const EMBED_ZSHRC: &str = include_str!("./.zshrc");
const EMBED_OS_RELEASE: &str = include_str!("./os-release");
const PACKAGES: &[&str] = &[
    "linux", "linux-headers", "linux-firmware", "lkpm", "lkchroot", "lkstrap",
    "systemd", "dbus", "glibc", "busybox", "zsh", "sudo", "efibootmgr",
    "networkmanager", "modemmanager", "usb_modeswitch", "inetutils", "bash",
    "nano", "vim", "grub", "libverto", "wget", "curl", "git", "which", "man-db",
    "man-pages", "lkinit", "util-linux", "coreutils", "findutils", "sed", "grep",
    "kmod", "e2fsprogs", "iputils", "gptfdisk", "parted", "dosfstools", "btrfs-progs",
    "xfsprogs", "ca-certificates", "libnghttp3", "libnghttp2", "libpsl", "libidn2",
    "brotli", "memtest86+",
];

fn print_info(msg: &str) { println!("{}[i]{} {}", CYAN, RESET, msg); }
fn print_success(msg: &str) { println!("{}[+]{} {}{}{}", CYAN, RESET, GREEN, msg, RESET); }
fn print_error(msg: &str) { println!("{}[-]{} {}{}{}", CYAN, RESET, RED, msg, RESET); }

fn check_host_dependencies() -> Result<(), String> {
    print_info("Checking dependencies for ISO generation....");
    let required_tools = ["grub-mkrescue", "xorriso", "mformat", "mkfs.fat"];
    for tool in &required_tools {
        let check = Command::new("which").arg(tool).output();
        match check {
            Ok(out) if out.status.success() => {},
            _ => return Err(format!("'{}' missing! Please install {} on your system to run liskaiso.", tool, tool)),
        }
    }
    let efi_dir1 = Path::new("/usr/lib/grub/x86_64-efi");
    let efi_dir2 = Path::new("/usr/share/grub/x86_64-efi");
    if !efi_dir1.exists() && !efi_dir2.exists() {
        return Err("GRUB x86_64-efi modules missing! Please install GRUB UEFI on your system.".into());
    }
    let bios_dir1 = Path::new("/usr/lib/grub/i386-pc");
    let bios_dir2 = Path::new("/usr/share/grub/i386-pc");
    if !bios_dir1.exists() && !bios_dir2.exists() {
        return Err("GRUB i386-pc modules missing! Please install GRUB BIOS on your system.".into());
    }
    print_success("All build dependencies satisfied.");
    Ok(())
}

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

fn run_lkpm_smart_timeout(cmd: &str, args: &[&str]) -> Result<(), String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
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
    let strip_ansi_stdout = strip_ansi;
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let raw = line.trim_end();
                    let l = strip_ansi_stdout(raw).replace('\r', "");
                    if l.contains("Initialize the operation") {
                        let _ = tx_clone.send(ReaderMsg::OperationStarted);
                    }
                    println!("{}", l);
                }
                Err(_) => break,
            }
        }
    });
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let raw = line.trim_end();
                    let l = strip_ansi(raw).replace('\r', "");
                    if l.contains("Initialize the operation") {
                        let _ = tx_clone_err.send(ReaderMsg::OperationStarted);
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

fn build_iso(workspace: &Path) -> Result<PathBuf, String> {
    let root = workspace.join("airootfs");
    let iso_root = workspace.join("iso_root");
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&iso_root).ok();
    fs::create_dir_all(&root).ok();
    fs::create_dir_all(&iso_root.join("boot")).ok();
    print_info("Building Liska Linux ISO....");
    install_package_pool(&root, PACKAGES)?;
    let sbin_dir = root.join("sbin");
    let usr_sbin_dir = root.join("usr/sbin");
    fs::create_dir_all(&sbin_dir).ok();
    fs::create_dir_all(&usr_sbin_dir).ok();
    let systemctl_target = root.join("usr/bin/systemctl");
    for cmd in &["reboot", "shutdown", "poweroff", "halt"] {
        let sbin_link = sbin_dir.join(cmd);
        let usr_sbin_link = usr_sbin_dir.join(cmd);
        let _ = fs::remove_file(&sbin_link);
        let _ = fs::remove_file(&usr_sbin_link);
        let _ = symlink(&systemctl_target, &sbin_link);
        let _ = symlink(&systemctl_target, &usr_sbin_link);
    }
    print_info("Enabling systemd core services....");
    let multi_user_wants = root.join("etc/systemd/system/multi-user.target.wants");
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
        if !symlink_path.exists() && root.join(target_path.trim_start_matches('/')).exists() {
            let _ = symlink(target_path, &symlink_path);
        }
    }
    let global_zsh_dir = root.join("etc/zsh");
    fs::create_dir_all(&global_zsh_dir).ok();
    let custom_zshrc_content = 
        "export TERM=xterm-256color\n\
         export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\n\
        ";
    fs::write(global_zsh_dir.join("zshrc"), custom_zshrc_content).map_err(|e| e.to_string())?;
    let root_zshrc = root.join("root/.zshrc");
    fs::create_dir_all(root.join("root")).ok();
    let local_zshrc = PathBuf::from("src/.zshrc");
    if local_zshrc.exists() {
        fs::copy(&local_zshrc, &root_zshrc).map_err(|e| e.to_string())?;
        print_info("Integrated custom src/.zshrc to /root/.zshrc....");
    } else {
        fs::write(&root_zshrc, EMBED_ZSHRC).map_err(|e| e.to_string())?;
        print_info("Installed embedded .zshrc to /root/.zshrc....");
    }
    let passwd_path = root.join("etc/passwd");
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
    let profile_path = root.join("etc/profile");
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
    let system_units_dir = root.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&system_units_dir).ok();
    let target_getty = root.join("lib/systemd/system/getty@.service");
    let link_getty = system_units_dir.join("getty@tty1.service");
    if target_getty.exists() && !link_getty.exists() {
        let _ = symlink("../getty@.service", &link_getty);
    }
    print_info("Configuring autologin for tty1....");
    let getty_override_dir = root.join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&getty_override_dir).ok();
    let autologin_conf = 
        "[Service]\n\
         ExecStart=\n\
         ExecStart=-/sbin/agetty --autologin root --noclear %I $TERM\n\
        ";
    fs::write(getty_override_dir.join("override.conf"), autologin_conf)
        .map_err(|e| e.to_string())?;
    print_info("Setting default systemd timezone to UTC....");
    let localtime_path = root.join("etc/localtime");
    let _ = fs::remove_file(&localtime_path);
    let _ = symlink("/usr/share/zoneinfo/UTC", &localtime_path);
    let _ = fs::write(root.join("etc/timezone"), "UTC\n");
    let mirrorlist_src = PathBuf::from("src/mirrorlist");
    fs::create_dir_all(root.join("etc/lkpm.d")).ok();
    let _ = fs::copy(&mirrorlist_src, root.join("etc/lkpm.d/mirrorlist"));
    print_info("Copied lkpm mirrorlist configuration");
    let os_release_src = PathBuf::from("src/os-release");
    fs::create_dir_all(root.join("etc")).ok();
    if os_release_src.exists() {
        fs::copy(&os_release_src, root.join("etc/os-release")).map_err(|e| e.to_string())?;
        print_info("Copied os-release from src/os-release.");
    } else {
        fs::write(root.join("etc/os-release"), EMBED_OS_RELEASE).map_err(|e| e.to_string())?;
        print_info("Installed embedded os-release.");
    }
    let kernel_src = root.join("boot/vmlinuz-linux");
    if !kernel_src.exists() {
        return Err("FATAL: vmlinuz-linux not found in rootfs!".into());
    }
    fs::copy(&kernel_src, iso_root.join("boot/vmlinuz-linux")).map_err(|e| e.to_string())?;
    let memtest86_src = root.join("boot/memtest86+/memtest.bin");
    if memtest86_src.exists() {
        let memtest_dst_dir = iso_root.join("boot/memtest86+");
        fs::create_dir_all(&memtest_dst_dir).ok();
        let _ = fs::copy(&memtest86_src, memtest_dst_dir.join("memtest.bin"));
    }
    let sysctl_dir = root.join("etc/sysctl.d");
    fs::create_dir_all(&sysctl_dir).ok();
    let _ = fs::write(
        sysctl_dir.join("20-quiet-printk.conf"),
        "kernel.printk = 3 3 3 3\n"
    );
    generate_pure_initramfs(&root, &iso_root)?;
    let squash_target = iso_root.join("liskafs.sfs");
    print_info("Compressing filesystem into liskafs.sfs....");
    run_command("mksquashfs", &[
        root.to_str().unwrap(),
        squash_target.to_str().unwrap(),
        "-comp", "zstd",
        "-noappend",
    ])?;
    write_grub_cfg(&iso_root.join("boot/grub/grub.cfg"))?;
    let iso_path = workspace.join("liskalinux-x86_64.iso");
    print_info(&format!("Building ISO: {}", iso_path.display()));
    run_command("grub-mkrescue", &["-o", iso_path.to_str().unwrap(), iso_root.to_str().unwrap()])?;
    print_success(&format!("Successfully built Liska Linux ISO at {}.", iso_path.display()));
    Ok(iso_path)
}

fn generate_pure_initramfs(rootfs: &Path, iso_root: &Path) -> Result<(), String> {
    print_info("Calling lkinit to generate initramfs....");
    let target_img = iso_root.join("boot/initramfs-liska.img");
    run_command("lkinit", &[
        "--root", rootfs.to_str().unwrap(),
        "--output", target_img.to_str().unwrap(),
        "--iso"
    ])?;
    Ok(())
}

fn write_grub_cfg(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let content = 
        "set timeout=5\n\
         set default=0\n\
         \n\
         menuentry \"Liska Linux x86_64\" {\n\
             linux /boot/vmlinuz-linux rw console=tty1 loglevel=3 audit=0 systemd.show_status=1 quiet cow_spacesize=2G\n\
             initrd /boot/initramfs-liska.img\n\
         }\n\
         \n\
         menuentry \"Memtest86 Utility\" {\n\
             insmod part_gpt\n\
             insmod fat\n\
             set root='hd0,gpt1'\n\
             chainloader /EFI/memtest/memtest86.efi\n\
         }\n\
         \n\
         menuentry \"UEFI Firmware Settings\" --class efi {\n\
             fwsetup\n\
         }\n\
         ";
    fs::write(path, content).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("");
        println!("-----------------------------------");
        println!("::: [ Liska Linux ISO Builder ] :::");
        println!("-----------------------------------");
        println!("Usage: sudo liskaiso");
        println!("");
        return;
    }
    if unsafe { libc::geteuid() } != 0 {
        print_error("Root permission required. Use 'sudo' for this operation.");
        exit(1);
    }
    check_host_dependencies()?;
    let workspace = PathBuf::from("/home/liskaiso-workspace");
    fs::create_dir_all(&workspace).ok();
    if let Err(e) = build_iso(&workspace) {
        print_error(&format!("Failed to build Liska Linux ISO: {}", e));
        exit(1);
    }
}