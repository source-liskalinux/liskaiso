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

const EMBED_ZSHRC_CLI: &str = include_str!("./.zshrc-cli");
const EMBED_ZSHRC_HYPRLAND: &str = include_str!("./.zshrc-hyprland");
const EMBED_ZSHRC: &str = "export TERM=xterm-256color\nPROMPT='%F{cyan}[liskalinux %~]# %f'\n";
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
    title: "Liska Linux CLI",
    packages: &[
        "linux", "linux-headers", "linux-firmware", "lkpm", "lkchroot", "lkstrap",
        "systemd", "dbus", "glibc", "busybox", "zsh", "sudo", "efibootmgr",
        "networkmanager", "modemmanager", "usb_modeswitch", "inetutils", "bash",
        "nano", "vim", "grub", "libverto", "wget", "curl", "git", "which", "man-db",
        "man-pages", "mkinitcpio", "util-linux", "coreutils", "findutils", "sed", "grep",
    ],
};

const HYPRLAND_EDITION: Edition = Edition {
    id: "hyprland",
    title: "Liska Linux Hyprland",
    packages: &[
        "linux", "linux-headers", "linux-firmware", "lkpm", "lkchroot", "lkstrap",
        "systemd", "dbus", "glibc", "busybox", "zsh", "sudo", "efibootmgr",
        "networkmanager", "modemmanager", "usb_modeswitch", "inetutils", "bash",
        "nano", "vim", "grub", "libverto", "wget", "curl", "git", "which", "man-db",
        "man-pages", "mkinitcpio", "util-linux", "coreutils", "findutils", "sed", "grep",
        "hyprland", "wayland", "wlroots", "mako", "waybar", "pipewire", "pipewire-pulse", 
        "alacritty", "firefox", "xwayland", "hyprpaper", "rofi", "zenity", "polkit", 
        "polkit-gnome", "calamares", "seatd", "mesa", "libdrm", "egl-wayland", "fastfetch",
    ],
};

fn apply_dotfiles(edition_root: &Path) -> Result<(), String> {
    print_info("Injecting dotfiles and calamares configs....");
    let src_config = PathBuf::from("src/.config");
    let skel_config = edition_root.join("etc/skel/.config");
    let root_config = edition_root.join("root/.config");
    if src_config.exists() {
        fs::create_dir_all(&skel_config).ok();
        fs::create_dir_all(&root_config).ok();
        let _ = run_command("cp", &["-rf", src_config.join(".").to_str().unwrap(), skel_config.to_str().unwrap()]);
        let _ = run_command("cp", &["-rf", src_config.join(".").to_str().unwrap(), root_config.to_str().unwrap()]);
        let _ = run_command("chmod", &["-R", "+x", skel_config.join("hypr/scripts").to_str().unwrap()]);
        let _ = run_command("chmod", &["-R", "+x", root_config.join("hypr/scripts").to_str().unwrap()]);
    }
    let src_calamares = PathBuf::from("src/etc/calamares");
    let target_calamares = edition_root.join("etc/calamares");
    if src_calamares.exists() {
        fs::create_dir_all(&target_calamares).ok();
        let _ = run_command("cp", &["-rf", src_calamares.join(".").to_str().unwrap(), target_calamares.to_str().unwrap()]);
    }
    print_info("Dotfiles and calamares configs injected successfully.");
    Ok(())
}

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
        format!("src/.zshrc-{}", edition.id),
        format!("src/.zshrc_{}", edition.id),
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
        let content = match edition.id {
            "cli" => EMBED_ZSHRC_CLI,
            "hyprland" => EMBED_ZSHRC_HYPRLAND,
            _ => EMBED_ZSHRC,
        };
        fs::write(&root_zshrc, content).map_err(|e| e.to_string())?;
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
    let auto_zsh_script = "
    if [ -t 1 ] && [ -n \"$PS1\" ] && [ -x /usr/bin/zsh ] && [ \"$(basename \"$SHELL\")\" != \"zsh\" ]; then
        export SHELL=/usr/bin/zsh
        export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
        exec /usr/bin/zsh -l
    fi
    ";
    if let Ok(mut current_profile) = fs::read_to_string(&profile_path) {
        current_profile.push_str(auto_zsh_script);
        let _ = fs::write(&profile_path, current_profile);
    } else {
        let _ = fs::write(&profile_path, auto_zsh_script);
    }
    print_info("Injected zsh to /etc/profile.");
    if edition.id == "hyprland" {
        print_info("Configuring direct autostart and seat management for Hyprland....");
        let seatd_wants = edition_root.join("etc/systemd/system/multi-user.target.wants");
        fs::create_dir_all(&seatd_wants).ok();
        let seatd_service_target = "/usr/lib/systemd/system/seatd.service";
        if edition_root.join("usr/lib/systemd/system/seatd.service").exists() {
            let _ = std::os::unix::fs::symlink(seatd_service_target, seatd_wants.join("seatd.service"));
        }
        let _ = Command::new("sh")
            .args(&["-c", &format!("chroot {} passwd -d root", edition_root.display())])
            .output();
        let _ = Command::new("sh")
            .args(&["-c", &format!("echo 'root:root' | chroot {} chpasswd", edition_root.display())])
            .output();
        let root_dir = edition_root.join("root");
        fs::create_dir_all(&root_dir).ok();
        let start_hypr_script = 
        "#!/bin/sh\n\
         export XDG_SESSION_TYPE=wayland\n\
         export XDG_SESSION_DESKTOP=Hyprland\n\
         export XDG_CURRENT_DESKTOP=Hyprland\n\
         export XDG_RUNTIME_DIR=\"/run/user/0\"\n\
         export LIBSEAT_BACKEND=builtin\n\
         export WLR_NO_HARDWARE_CURSORS=1\n\
         export LIBGL_ALWAYS_SOFTWARE=1\n\
         export WLR_RENDERER_ALLOW_SOFTWARE=1\n\
         export WLR_RENDERER=pixman\n\
         mkdir -p \"$XDG_RUNTIME_DIR\"\n\
         chmod 0700 \"$XDG_RUNTIME_DIR\"\n\
         if [ -z \"$DBUS_SESSION_BUS_ADDRESS\" ]; then\n\
             eval $(dbus-launch --sh-syntax --exit-with-session)\n\
         fi\n\
         dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE >/dev/null 2>&1 || true\n\
         if command -v Hyprland >/dev/null 2>&1; then\n\
             echo \"[+] Booting Liska Linux Hyprland....\"\n\
             exec dbus-run-session start-hyprland --i-am-really-stupid\n\
         else\n\
             echo \"[!] CRITICAL: Hyprland not found on /usr/bin!\"\n\
             sleep 5\n\
         fi\n\
        ";
        let start_hypr_path = edition_root.join("usr/bin/start-hypr");
        fs::write(&start_hypr_path, start_hypr_script).ok();
        let _ = run_command("chmod", &["+x", start_hypr_path.to_str().unwrap()]);
        let zprofile_content = 
        "if [ \"$(tty)\" = \"/dev/tty1\" ] || [ \"$(tty)\" = \"tty1\" ]; then\n\
            if [ -z \"$DISPLAY\" ] && [ -z \"$WAYLAND_DISPLAY\" ]; then\n\
                exec /usr/bin/start-hypr\n\
            fi\n\
        fi\n\
        ";
        fs::write(root_dir.join(".zprofile"), zprofile_content).ok();
        fs::write(root_dir.join(".bash_profile"), zprofile_content).ok();
        let zlogin_content = 
        "if [ -f ~/.zprofile ]; then\n\
             . ~/.zprofile\n\
         fi\n\
        ";
        fs::write(root_dir.join(".zlogin"), zlogin_content).ok();
        apply_dotfiles(&edition_root)?;
    }
    let os_release_src = PathBuf::from("src/os-release");
    fs::create_dir_all(edition_root.join("etc")).ok();
    if os_release_src.exists() {
        let os_release_content = fs::read_to_string(&os_release_src).map_err(|e| e.to_string())?;
        let os_release_content = os_release_content.replace("{edition name}", edition.title);
        fs::write(edition_root.join("etc/os-release"), os_release_content).map_err(|e| e.to_string())?;
        print_info(&format!("Integrated os-release for {}.", edition.title));
    } else {
        let os_release_content = EMBED_OS_RELEASE.replace("{edition name}", edition.title);
        fs::write(edition_root.join("etc/os-release"), os_release_content).map_err(|e| e.to_string())?;
        print_info(&format!("Installed embedded os-release for {}.", edition.title));
    }
    let system_units_dir = edition_root.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&system_units_dir).ok();
    let target_getty = edition_root.join("lib/systemd/system/getty@.service");
    let link_getty = system_units_dir.join("getty@tty1.service");
    if target_getty.exists() && !link_getty.exists() {
        let _ = std::os::unix::fs::symlink("../getty@.service", &link_getty);
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
    print_info("Generating Liska Linux initramfs....");    
    let rootfs_mod_dir = rootfs.join("usr/lib/modules");
    let mut kernel_version = String::new();
    if rootfs_mod_dir.exists() {
        if let Ok(entries) = fs::read_dir(&rootfs_mod_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    kernel_version = entry.file_name().to_string_lossy().into_owned();
                    break;
                }
            }
        }
    }
    if kernel_version.is_empty() {
        return Err("FATAL: kernel version not found in rootfs/usr/lib/modules!".into());
    }
    let temp_ramdisk = PathBuf::from("/tmp/liskaiso_initramfs");
    fs::remove_dir_all(&temp_ramdisk).ok();
    let dirs = &[
        "bin", "sbin", "dev", "proc", "sys", "root", "run", 
        "usr/bin", "usr/sbin", "lib", "lib64", 
        "usr/lib/modules"
    ];
    for dir in dirs {
        fs::create_dir_all(temp_ramdisk.join(dir)).ok();
    }
    let dst_mod_dir = temp_ramdisk.join("usr/lib/modules");
    let _ = run_command("cp", &["-ax", rootfs_mod_dir.to_str().unwrap(), temp_ramdisk.join("usr/lib/").to_str().unwrap()]);
    print_info(" Uncompressing all kernel modules for initramfs compatibility....");
    let target_kernel_dir = dst_mod_dir.join(&kernel_version);
    let _ = run_command("sh", &["-c", &format!("find {} -name '*.ko.zst' -exec unzstd --rm -f {{}} \\;", target_kernel_dir.display())]);
    let busybox_candidates = &[rootfs.join("bin/busybox"), rootfs.join("usr/bin/busybox")];
    let mut busybox_src = None;
    for candidate in busybox_candidates {
        if candidate.exists() { busybox_src = Some(candidate); break; }
    }
    let busybox_path = match busybox_src {
        Some(path) => path,
        None => return Err("CRITICAL: busybox not found in rootfs!".into()),
    };
    fs::copy(busybox_path, temp_ramdisk.join("bin/busybox")).ok();
    let liska_libs = &[
        ("usr/lib/ld-linux-x86-64.so.2", "lib64/ld-linux-x86-64.so.2"),
        ("usr/lib/libc.so.6", "lib/libc.so.6"),
    ];
    for (src_lib, dst_lib) in liska_libs {
        let rootfs_lib_path = rootfs.join(src_lib);
        if rootfs_lib_path.exists() {
            fs::copy(&rootfs_lib_path, temp_ramdisk.join(dst_lib)).ok();
        } else {
            let fallback_path = rootfs.join(src_lib.replace("usr/", ""));
            if fallback_path.exists() {
                fs::copy(&fallback_path, temp_ramdisk.join(dst_lib)).ok();
            }
        }
    }
    for link in &["sh", "mount", "umount", "sleep", "switch_root", "init", "mdev", "insmod", "find"] {
        let link_path = temp_ramdisk.join("bin").join(link);
        let _ = fs::remove_file(&link_path);
        let _ = run_command("ln", &["-sf", "busybox", link_path.to_str().unwrap()]);
    }
    let _ = run_command("ln", &["-sf", "../bin/busybox", temp_ramdisk.join("sbin/init").to_str().unwrap()]);
    let _ = run_command("ln", &["-sf", "../bin/busybox", temp_ramdisk.join("sbin/switch_root").to_str().unwrap()]);
    let getty_dir = rootfs.join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&getty_dir).ok();
    let override_conf = getty_dir.join("override.conf");
    let getty_content = "[Service]\n\
        ExecStart=\n\
        ExecStart=-/usr/sbin/agetty --autologin root --noclear %I $TERM\n";
    fs::write(&override_conf, getty_content).ok();
    let localtime_path = rootfs.join("etc/localtime");
    if localtime_path.exists() {
        let _ = fs::remove_file(&localtime_path);
    }
    let _ = std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime_path);
    let raw_init_script = format!(
        "#!/bin/sh\n\
        export PATH=/usr/bin:/bin:/usr/sbin:/sbin\n\
        CYAN='\\033[36m'\n\
        GREEN='\\033[92m'\n\
        RED='\\033[31m'\n\
        BOLD='\\033[1m'\n\
        RESET='\\033[0m'\n\
        \n\
        clear\n\
        printf \"\\n\"\n\
        printf \"${{CYAN}}${{BOLD}}::: [ LISKA LINUX INITRAMFS ] :::${{RESET}}\\n\"\n\
        printf \"\\n\"\n\
        printf \"${{CYAN}}[i]${{RESET}} Initializing Liska Linux initramfs....\\n\"\n\
        \n\
        mkdir -p /proc /sys /dev /new_root /src_sfs /cow /run/liska/bootmnt\n\
        mount -t proc proc /proc\n\
        mount -t sysfs sys /sys\n\
        mount -t devtmpfs dev /dev\n\
        \n\
        echo 1 > /proc/sys/kernel/printk\n\
        \n\
        printf \"${{CYAN}}[i]${{RESET}} Waiting for storage drives to settle....\\n\"\n\
        \n\
        FOUND=0\n\
        for i in $(seq 1 15); do\n\
            /bin/mdev -s 2>/dev/null || true\n\
            \n\
            for dev in /dev/sd* /dev/nvme* /dev/vd* /dev/mmcblk* /dev/sr*; do\n\
                if [ -b \"$dev\" ]; then\n\
                    mount -r \"$dev\" /run/liska/bootmnt 2>/dev/null\n\
                    if [ -f /run/liska/bootmnt/liskafs.sfs ]; then\n\
                        FOUND=1\n\
                        printf \"${{CYAN}}[+]${{RESET}} ${{GREEN}}${{BOLD}}Liska Linux found on $dev! Initializing squashfs.${{RESET}}\\n\"\n\
                        break 2\n\
                    fi\n\
                    umount /run/liska/bootmnt 2>/dev/null\n\
                fi\n\
            done\n\
            sleep 1\n\
        done\n\
        \n\
        if [ \"$FOUND\" -eq 0 ]; then\n\
            printf \"${{CYAN}}[-]${{RESET}} ${{RED}}${{BOLD}}CRITICAL: Could not find Liska Linux on any drive! Falling back to shell.${{RESET}}\\n\"\n\
            exec /bin/sh\n\
        fi\n\
        \n\
        echo 4 > /proc/sys/kernel/printk\n\
        \n\
        mount -t squashfs /run/liska/bootmnt/liskafs.sfs /src_sfs\n\
        mount -t tmpfs tmpfs /cow\n\
        mkdir -p /cow/upper /cow/work\n\
        mount -t overlay overlay -o lowerdir=/src_sfs,upperdir=/cow/upper,workdir=/cow/work /new_root\n\
        \n\
        mkdir -p /new_root/proc /new_root/sys /new_root/dev /new_root/run\n\
        \n\
        mount --move /proc /new_root/proc\n\
        mount --move /sys /new_root/sys\n\
        mount --move /dev /new_root/dev\n\
        \n\
        printf \"${{CYAN}}[i]${{RESET}} Searching systemd....\\n\"\n\
        SYSTEMD_PATH=$(find /new_root/usr/lib/systemd/systemd /new_root/lib/systemd/systemd /new_root/usr/bin/systemd 2>/dev/null | head -n 1)\n\
        \n\
        if [ -n \"$SYSTEMD_PATH\" ]; then\n\
            TARGET=\"${{SYSTEMD_PATH#/new_root}}\"\n\
            printf \"${{CYAN}}[+]${{RESET}} ${{GREEN}}${{BOLD}}Systemd was found! Initializing systemd.${{RESET}}\\n\"\n\
            exec switch_root /new_root \"$TARGET\" --show-status=1\n\
        else\n\
            printf \"${{CYAN}}[-]${{RESET}} ${{RED}}${{BOLD}}CRITICAL: Systemd not found! Falling back to bash shell.${{RESET}}\\n\"\n\
            exec /bin/sh\n\
        fi\n",
    );
    let init_path = temp_ramdisk.join("init");
    let _ = fs::remove_file(&init_path); 
    fs::write(&init_path, raw_init_script).map_err(|e| e.to_string())?;
    run_command("chmod", &["+x", init_path.to_str().unwrap()])?;
    let target_img = iso_root.join("boot/initramfs-liska.img");
    print_info("Packing Liska Linux initramfs....");
    run_command("sh", &vec![
        "-c",
        &format!(
            "cd {} && find . -mindepth 1 | cpio -H newc -o --quiet | zstd -19 -T0 > {}",
            temp_ramdisk.display(),
            target_img.display()
        )
    ])?;
    fs::remove_dir_all(&temp_ramdisk).ok();
    print_success("Liska Linux initramfs generation completed!");
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
        println!("> --hyprland               build Hyprland edition");
        println!("> --all                    build all editions");
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
    let editions = if args.len() > 1 && args[1] == "--hyprland" {
        vec![&HYPRLAND_EDITION]
    } else if args.len() > 1 && args[1] == "--all" {
        vec![&CLI_EDITION, &HYPRLAND_EDITION]
    } else if args.len() > 1 && args[1] == "--cli" {
        vec![&CLI_EDITION]
    } else {
        print_error("No valid edition specified. Use --help for usage.");
        exit(1);
    };
    for edition in editions {
        match build_edition(edition, &workspace) {
            Ok(_) => {},
            Err(e) => {
                print_error(&format!("Failed to build {}: {}", edition.id, e));
                exit(1);
            }
        }
    }
    print_success("All editions has been built successfully!");
}
