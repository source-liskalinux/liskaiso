use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use colored::*;

const PACKAGES: &[&str] = &[
    "linux", "linux-firmware", "lkfs", "lkpm", "lksystem",
    "liska-install-scripts", "dbus", "glibc", "busybox", "zsh", "efibootmgr",
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
    let required_tools = ["grub-mkrescue", "xorriso", "mformat", "mkfs.fat", "mksquashfs"];
    for tool in &required_tools {
        let check = Command::new("which").arg(tool).output();
        match check {
            Ok(out) if out.status.success() => {},
            _ => return Err(format!("{} is missing! Please install {} on your system to run liskaiso.", tool, tool)),
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
        .map_err(|err| format!("Could not start {}! Err: {}.", cmd, err))?;
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
    run_command("lkpm", &["-i", "--root", root.to_str().unwrap(), "--noconfirm", "filesystem", "iana-etc"])?;
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
    let passwd_path = root.join("etc/passwd");
    if passwd_path.exists() {
        if let Ok(content) = fs::read_to_string(&passwd_path) {
            let mut new_lines = Vec::new();
            for line in content.lines() {
                if line.starts_with("root:") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 6 {
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
    let shells_path = root.join("etc/shells");
    let shells_content = "/bin/sh\n/bin/bash\n/bin/zsh\n/usr/bin/zsh\n";
    let _ = fs::write(&shells_path, shells_content);
    info("Configuring ISO autologin....");
    let autologin_script = root.join("usr/bin/autologin");
    let _ = fs::write(&autologin_script, "#!/bin/sh\nexec /bin/login -f root\n");
    let _ = run_command("chmod", &["+x", autologin_script.to_str().unwrap()]);
    let getty_service_path = root.join("etc/lksystem/services/agetty-tty1/run");
    if getty_service_path.exists() {
        if let Ok(content) = fs::read_to_string(&getty_service_path) {
            let new_content = content.replace(
                "exec chpst -P agetty 38400 tty1 linux",
                "exec chpst -P agetty -n -l /usr/bin/autologin 38400 tty1 linux"
            );
            fs::write(&getty_service_path, new_content).map_err(|e| e.to_string())?;
        }
    }
    let getty2 = root.join("etc/lksystem/services/agetty-tty2");
    let getty3 = root.join("etc/lksystem/services/agetty-tty3");
    let getty4 = root.join("etc/lksystem/services/agetty-tty4");
    let getty5 = root.join("etc/lksystem/services/agetty-tty5");
    let getty6 = root.join("etc/lksystem/services/agetty-tty6");
    let _ = fs::remove_dir_all(&getty2);
    let _ = fs::remove_dir_all(&getty3);
    let _ = fs::remove_dir_all(&getty4);
    let _ = fs::remove_dir_all(&getty5);
    let _ = fs::remove_dir_all(&getty6);
    info("Setting default ISO timezone to UTC....");
    let localtime_path = root.join("etc/localtime");
    let _ = fs::remove_file(&localtime_path);
    let _ = symlink("/usr/share/zoneinfo/UTC", &localtime_path);
    let _ = fs::write(root.join("etc/timezone"), "UTC\n");
    info("Setting up ca-certificates....");
    let _ = run_command("lkchroot", &[root.to_str().unwrap(), "update-ca-trust"]);
    let kernel_src = root.join("boot/vmlinuz-linux");
    if !kernel_src.exists() {
        return Err("FATAL: vmlinuz-linux not found in rootfs!".into());
    }
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
        "--output", target_img.to_str().unwrap()
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
        println!("Usage: liskaiso <options>");
        println!("> -w | --workspace <path>     set workspace directory (default: ./iso-workspace)");
        println!("> -v | --version <ver>        set ISO version (default: 2026)");
        println!("> /etc/liskaiso               liskaiso workspace directory template");
        println!("");
        return;
    }
    if unsafe { libc::getuid() } != 0 {
        error("Operation not permitted (os error 1)!");
        exit(1);
    }
    let mut version = String::from("2026");
    let mut workspace_arg = String::from("iso-workspace");
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if (arg == "--version" || arg == "-v") && i + 1 < args.len() {
            version = args[i + 1].clone();
            i += 1;
        } else if (arg == "--workspace" || arg == "-w") && i + 1 < args.len() {
            workspace_arg = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }
    if let Err(e) = check_host_dependencies() {
        error(&format!("{}", e));
        exit(1);
    }
    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error(&format!("Failed to get current directory! Err: {}", e));
            exit(1);
        }
    };
    let target_path = PathBuf::from(&workspace_arg);
    let workspace = if target_path.is_absolute() {
        target_path
    } else {
        current_dir.join(target_path)
    };
    if let Err(e) = fs::create_dir_all(&workspace) {
        error(&format!("Failed to create workspace directory! Err: {}", e));
        exit(1);
    }
    if let Err(e) = build_iso(&workspace, &version) {
        error(&format!("Failed to build Liska Linux ISO! Err: {}", e));
        exit(1);
    }
}
