use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use colored::*;

const PACKAGES: &[&str] = &[
    "linux", "linux-firmware", "lkfs", "lkpm", "lksystem",
    "liska-install-scripts", "dbus", "glibc", "busybox", "zsh", "sudo", "efibootmgr",
    "networkmanager", "modemmanager", "usb_modeswitch", "inetutils", "bash",
    "nano", "vim", "grub", "wget", "curl", "git", "which", "man-db", "man-pages",
    "util-linux", "coreutils", "findutils", "sed", "grep", "kmod", "e2fsprogs",
    "iputils", "gptfdisk", "parted", "dosfstools", "btrfs-progs", "xfsprogs",
    "ca-certificates", "libnghttp3", "libnghttp2", "libpsl", "libidn2", "brotli",
    "memtest86+-efi", "krb5"
];

fn info(msg: &str) { println!("{} {}", "::: [ LISKAISO ] ::: (i) >".bright_cyan(), msg); }
fn success(msg: &str) { println!("{} {}", "::: [ LISKAISO ] ::: (✓) >".bright_green(), msg.bright_green()); }
fn error(msg: &str) { println!("{} {}", "::: [ LISKAISO ] ::: (✗) >".bright_red(), msg.bright_red()); }

fn check_host_dependencies() -> Result<(), String> {
    info("Checking liskaiso dependencies....");
    let required_tools = ["grub-mkrescue", "xorriso", "mformat", "mkfs.fat"];
    for tool in &required_tools {
        let check = Command::new("which").arg(tool).output();
        match check {
            Ok(out) if out.status.success() => {},
            _ => return Err(format!("'{}' is missing! Please install {} on your system to run liskaiso.", tool, tool)),
        }
    }
    let has_efi64 = Path::new("/usr/lib/grub/x86_64-efi").exists() || Path::new("/usr/share/grub/x86_64-efi").exists();
    let has_efi32 = Path::new("/usr/lib/grub/i386-efi").exists() || Path::new("/usr/share/grub/i386-efi").exists();
    if !has_efi64 && !has_efi32 {
        return Err("GRUB UEFI modules is missing! Please install GRUB package on your system to run liskaiso.".into());
    }
    success("All build dependencies satisfied.");
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

fn load_package_list(workspace: &Path) -> Vec<String> {
    let pkg_file = workspace.join("packages");
    if pkg_file.exists() {
        if let Ok(content) = fs::read_to_string(&pkg_file) {
            let pkgs: Vec<String> = content
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                .collect();
            if !pkgs.is_empty() {
                info(&format!("Loaded {} packages from {}", pkgs.len(), pkg_file.display()));
                return pkgs;
            }
        }
    }
    let default_content = PACKAGES.join("\n");
    let _ = fs::write(&pkg_file, default_content);
    info(&format!("Successfully loaded default package list at {}", pkg_file.display()));
    PACKAGES.iter().map(|s| s.to_string()).collect()
}

fn install_package_pool(root: &Path, packages: &[String]) -> Result<(), String> {
    let _ = run_command("lkpm", &["-r", "--root", root.to_str().unwrap()]);
    let mut args = vec![
        "-id".to_string(),
        "--root".to_string(),
        root.to_str().unwrap().to_string(),
        "--noconfirm".to_string(),
    ];
    args.extend(packages.iter().cloned());
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_command("lkpm", &args_ref)?;
    let kernel_check = root.join("boot/vmlinuz-linux");
    if !kernel_check.exists() {
        return Err("FATAL: vmlinuz-linux not found on rootfs!".to_string());
    }
    Ok(())
}

fn build_iso(workspace: &Path, version: &str) -> Result<PathBuf, String> {
    let root = workspace.join("airootfs");
    let iso_root = workspace.join("iso_root");
    fs::create_dir_all(&root).ok();
    fs::create_dir_all(&iso_root).ok();
    info("Building Liska Linux ISO....");
    let packages = load_package_list(workspace);
    install_package_pool(&root, &packages)?;
    let sbin_dir = root.join("sbin");
    let usr_sbin_dir = root.join("usr/sbin");
    fs::create_dir_all(&sbin_dir).ok();
    fs::create_dir_all(&usr_sbin_dir).ok();
    let systemctl_target = Path::new("/usr/bin/lksystemctl");
    for cmd in &["reboot", "shutdown", "poweroff", "halt"] {
        let sbin_link = sbin_dir.join(cmd);
        let usr_sbin_link = usr_sbin_dir.join(cmd);
        let _ = fs::remove_file(&sbin_link);
        let _ = fs::remove_file(&usr_sbin_link);
        let _ = symlink(systemctl_target, &sbin_link);
        let _ = symlink(systemctl_target, &usr_sbin_link);
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
    info("Configuring autologin....");
    let getty_override_dir = root.join("etc/lksystem/system/getty1.service.d");
    fs::create_dir_all(&getty_override_dir).ok();
    let autologin_conf = 
        "[Service]\n\
         ExecStart=\n\
         ExecStart=-/sbin/agetty --autologin root --noclear %I $TERM\n\
        ";
    fs::write(getty_override_dir.join("override.conf"), autologin_conf)
        .map_err(|e| e.to_string())?;
    info("Setting default timezone to UTC....");
    let localtime_path = root.join("etc/localtime");
    let _ = fs::remove_file(&localtime_path);
    let _ = symlink("/usr/share/zoneinfo/UTC", &localtime_path);
    let _ = fs::write(root.join("etc/timezone"), "UTC\n");
    let kernel_src = root.join("boot/vmlinuz-linux");
    if !kernel_src.exists() {
        return Err("FATAL: vmlinuz-linux not found in rootfs!".into());
    }
    let iso_boot_dir = iso_root.join("boot");
    fs::create_dir_all(&iso_boot_dir).map_err(|e| e.to_string())?;
    fs::copy(&kernel_src, iso_root.join("boot/vmlinuz-linux")).map_err(|e| e.to_string())?;
    let memtest86_efi = root.join("boot/memtest86+/memtest.efi");
    if memtest86_efi.exists() {
        let efi_dst = iso_root.join("EFI/memtest");
        fs::create_dir_all(&efi_dst).ok();
        let _ = fs::copy(&memtest86_efi, efi_dst.join("memtest86.efi"));
    }
    generate_pure_initramfs(&root, &iso_root)?;
    let squash_target = iso_root.join("liskafs.sfs");
    info("Compressing filesystem into liskafs.sfs....");
    run_command("mksquashfs", &[
        root.to_str().unwrap(),
        squash_target.to_str().unwrap(),
        "-comp", "zstd",
        "-noappend",
    ])?;
    let iso_filename = format!("liskalinux-{}-x86_64.iso", version);
    let iso_path = workspace.join(iso_filename);
    info(&format!("Building ISO: {}", iso_path.display()));
    run_command("grub-mkrescue", &["-o", iso_path.to_str().unwrap(), iso_root.to_str().unwrap()])?;
    success(&format!("Successfully built Liska Linux ISO at {}.", iso_path.display()));
    Ok(iso_path)
}

fn generate_pure_initramfs(rootfs: &Path, iso_root: &Path) -> Result<(), String> {
    info("Calling lkinit to generate initramfs....");
    let target_img = iso_root.join("boot/initramfs-liska.img");
    run_command("lkinit", &[
        "--root", rootfs.to_str().unwrap(),
        "--output", target_img.to_str().unwrap(),
        "--iso"
    ])?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("");
        println!("---------------------------------");
        println!("::: [ Liska ISO Builder (1) ] :::");
        println!("---------------------------------");
        println!("");
        println!("Usage: liskaiso --version=<version (default: 2026)>");
        println!("");
        return;
    }
    if unsafe { libc::geteuid() } != 0 {
        error("Root permission required. Use 'sudo' for this operation!");
        exit(1);
    }
    let mut version = String::from("2026");
    let mut i = 1;
    while i < args.len() {
        if args[i].starts_with("--version=") {
            version = args[i].trim_start_matches("--version=").to_string();
        } else if args[i] == "--version" && i + 1 < args.len() {
            version = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }
    let _ = check_host_dependencies();
    let workspace = PathBuf::from("/home/liskaiso-workspace");
    fs::create_dir_all(&workspace).ok();
    if let Err(e) = build_iso(&workspace, &version) {
        error(&format!("Failed to build Liska Linux ISO: {}", e));
        exit(1);
    }
}
