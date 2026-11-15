use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    for path in [
        "build.rs",
        "packaging/windows/app.manifest",
        "packaging/windows/app.rc",
        "packaging/windows/app.ico",
        "packaging/macos/Info.plist",
        "packaging/macos/entitlements.plist",
        "packaging/macos/AppIcon.icns",
        "ui/branding/logo.png",
    ] {
        println!("cargo:rerun-if-changed={}", path);
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        configure_windows_resources();
    }
}

fn configure_windows_resources() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    let rc_path = manifest_dir.join("packaging/windows/app.rc");
    let manifest_path = manifest_dir.join("packaging/windows/app.manifest");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".into());

    if target_env == "gnu" {
        let windres_candidates = [
            "windres".to_string(),
            format!("{}-w64-mingw32-windres", target_arch),
            "x86_64-w64-mingw32-windres".to_string(),
        ];
        for tool in windres_candidates {
            if tool_available(&tool, "--version") {
                let obj_out = out_dir.join("app_res.o");
                let status = Command::new(&tool)
                    .args(["-O", "coff", "-I"])
                    .arg(manifest_dir.join("packaging/windows"))
                    .arg(&rc_path)
                    .arg(&obj_out)
                    .status();
                if matches!(status, Ok(s) if s.success()) {
                    println!("cargo:rustc-link-arg-bins={}", obj_out.display());
                    return;
                }
            }
        }
        println!("cargo:warning=windres not found; Windows .rc resources skipped for GNU target.");
        return;
    }

    // MSVC / default Windows toolchain: emit hardening, version & checksum PE flags
    for arg in [
        "/RELEASE",
        "/VERSION:2.0",
        "/HIGHENTROPYVA",
        "/DYNAMICBASE",
        "/NXCOMPAT",
        "/CETCOMPAT",
        "/PDBALTPATH:%_PDB%",
    ] {
        println!("cargo:rustc-link-arg-bins={}", arg);
    }

    // Compile app.rc via rc.exe or llvm-rc when available
    let res_out = out_dir.join("app.res");
    if let Some(rc_tool) = find_windows_rc_tool(&target_arch) {
        let status = Command::new(&rc_tool)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res_out)
            .arg(&rc_path)
            .status();
        if matches!(status, Ok(s) if s.success()) {
            println!("cargo:rustc-link-arg-bins={}", res_out.display());
            return;
        }
    }

    // Pure-Rust Windows .res compiler fallback (for cross-compilation via cargo-xwin / lld-link)
    let ico_path = manifest_dir.join("packaging/windows/app.ico");
    if write_pure_rust_win32_res(&res_out, &ico_path, &manifest_path).is_ok() {
        println!("cargo:rustc-link-arg-bins={}", res_out.display());
        return;
    }

    // Final fallback: ask MSVC linker to embed the application manifest directly
    if manifest_path.exists() {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }
}

fn write_pure_rust_win32_res(
    res_out: &Path,
    ico_path: &Path,
    manifest_path: &Path,
) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    // Standard 32-byte null resource header at start of .res file
    push_res_entry(&mut buf, 0, 0, 0, 0, &[]);

    let mut wrote_any = false;

    // RT_VERSION = 16, VS_VERSION_INFO id = 1
    let ver_bytes = build_vs_version_info();
    if !ver_bytes.is_empty() {
        push_res_entry(&mut buf, 16, 1, 0x0409, 0x0030, &ver_bytes);
        wrote_any = true;
    }

    if manifest_path.exists() {
        let manifest_bytes = std::fs::read(manifest_path)?;
        if !manifest_bytes.is_empty() {
            // RT_MANIFEST = 24, CREATEPROCESS_MANIFEST_RESOURCE_ID = 1
            push_res_entry(&mut buf, 24, 1, 0x0409, 0x0030, &manifest_bytes);
            wrote_any = true;
        }
    }

    if ico_path.exists() {
        let ico = std::fs::read(ico_path)?;
        if ico.len() >= 6 {
            let reserved = u16::from_le_bytes([ico[0], ico[1]]);
            let ico_type = u16::from_le_bytes([ico[2], ico[3]]);
            let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
            if reserved == 0 && ico_type == 1 && count > 0 && ico.len() >= 6 + count * 16 {
                let mut grp_icon: Vec<u8> = Vec::with_capacity(6 + count * 14);
                grp_icon.extend_from_slice(&ico[0..6]);
                let mut valid = true;
                for i in 0..count {
                    let entry_off = 6 + i * 16;
                    let bytes_in_res = u32::from_le_bytes([
                        ico[entry_off + 8],
                        ico[entry_off + 9],
                        ico[entry_off + 10],
                        ico[entry_off + 11],
                    ]) as usize;
                    let img_offset = u32::from_le_bytes([
                        ico[entry_off + 12],
                        ico[entry_off + 13],
                        ico[entry_off + 14],
                        ico[entry_off + 15],
                    ]) as usize;
                    if img_offset.saturating_add(bytes_in_res) > ico.len() {
                        valid = false;
                        break;
                    }
                    let icon_id = (i + 1) as u16;
                    // RT_ICON = 3
                    push_res_entry(
                        &mut buf,
                        3,
                        icon_id,
                        0x0409,
                        0x1010,
                        &ico[img_offset..img_offset + bytes_in_res],
                    );
                    grp_icon.extend_from_slice(&ico[entry_off..entry_off + 12]);
                    grp_icon.extend_from_slice(&icon_id.to_le_bytes());
                }
                if valid {
                    // RT_GROUP_ICON = 14, MAINICON id = 1
                    push_res_entry(&mut buf, 14, 1, 0x0409, 0x1030, &grp_icon);
                    wrote_any = true;
                }
            }
        }
    }

    if !wrote_any {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no resources found",
        ));
    }
    std::fs::write(res_out, buf)
}

fn push_res_entry(
    buf: &mut Vec<u8>,
    res_type: u16,
    res_id: u16,
    lang: u16,
    mem_flags: u16,
    data: &[u8],
) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    buf.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize
    buf.extend_from_slice(&0xFFFFu16.to_le_bytes()); // TYPE ordinal marker
    buf.extend_from_slice(&res_type.to_le_bytes());
    buf.extend_from_slice(&0xFFFFu16.to_le_bytes()); // NAME ordinal marker
    buf.extend_from_slice(&res_id.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    buf.extend_from_slice(&mem_flags.to_le_bytes()); // MemoryFlags
    buf.extend_from_slice(&lang.to_le_bytes()); // LanguageId
    buf.extend_from_slice(&0u32.to_le_bytes()); // Version
    buf.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    buf.extend_from_slice(data);
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn push_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_utf16z(buf: &mut Vec<u8>, s: &str) {
    for c in s.encode_utf16() {
        push_u16(buf, c);
    }
    push_u16(buf, 0);
}

fn pad4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn build_version_string_entry(key: &str, val: &str) -> Vec<u8> {
    let mut b = Vec::new();
    let val_u16: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
    push_u16(&mut b, 0); // wLength placeholder
    push_u16(&mut b, val_u16.len() as u16); // wValueLength (WORDs)
    push_u16(&mut b, 1); // wType = text
    push_utf16z(&mut b, key);
    pad4(&mut b);
    for w in val_u16 {
        push_u16(&mut b, w);
    }
    let len = (b.len() as u16).to_le_bytes();
    b[0] = len[0];
    b[1] = len[1];
    b
}

fn build_string_table(lang_cp: &str, entries: &[(&str, &str)]) -> Vec<u8> {
    let mut b = Vec::new();
    push_u16(&mut b, 0);
    push_u16(&mut b, 0);
    push_u16(&mut b, 1);
    push_utf16z(&mut b, lang_cp);
    pad4(&mut b);
    for (k, v) in entries {
        pad4(&mut b);
        let child = build_version_string_entry(k, v);
        b.extend_from_slice(&child);
    }
    let len = (b.len() as u16).to_le_bytes();
    b[0] = len[0];
    b[1] = len[1];
    b
}

fn build_string_file_info(string_table: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    push_u16(&mut b, 0);
    push_u16(&mut b, 0);
    push_u16(&mut b, 1);
    push_utf16z(&mut b, "StringFileInfo");
    pad4(&mut b);
    b.extend_from_slice(string_table);
    let len = (b.len() as u16).to_le_bytes();
    b[0] = len[0];
    b[1] = len[1];
    b
}

fn build_var_file_info() -> Vec<u8> {
    let mut var_node = Vec::new();
    push_u16(&mut var_node, 0);
    push_u16(&mut var_node, 4); // 4 bytes for DWORD Translation pair
    push_u16(&mut var_node, 0); // binary
    push_utf16z(&mut var_node, "Translation");
    pad4(&mut var_node);
    push_u16(&mut var_node, 0x0409); // US English
    push_u16(&mut var_node, 0x04B0); // Unicode (UTF-16LE)
    let vlen = (var_node.len() as u16).to_le_bytes();
    var_node[0] = vlen[0];
    var_node[1] = vlen[1];

    let mut b = Vec::new();
    push_u16(&mut b, 0);
    push_u16(&mut b, 0);
    push_u16(&mut b, 1);
    push_utf16z(&mut b, "VarFileInfo");
    pad4(&mut b);
    b.extend_from_slice(&var_node);
    let len = (b.len() as u16).to_le_bytes();
    b[0] = len[0];
    b[1] = len[1];
    b
}

/// Builds a Win32 binary VS_VERSIONINFO (RT_VERSION = 16) resource block.
fn build_vs_version_info() -> Vec<u8> {
    let mut b = Vec::new();
    push_u16(&mut b, 0); // wLength placeholder
    push_u16(&mut b, 52); // wValueLength = sizeof(VS_FIXEDFILEINFO)
    push_u16(&mut b, 0); // wType = binary VS_FIXEDFILEINFO
    push_utf16z(&mut b, "VS_VERSION_INFO");
    pad4(&mut b);

    // VS_FIXEDFILEINFO (52 bytes)
    push_u32(&mut b, 0xFEEF_04BD); // dwSignature
    push_u32(&mut b, 0x0001_0000); // dwStrucVersion 1.0
    push_u32(&mut b, 0x0002_0000); // dwFileVersionMS 2.0
    push_u32(&mut b, 0x0000_0000); // dwFileVersionLS 0.0
    push_u32(&mut b, 0x0002_0000); // dwProductVersionMS 2.0
    push_u32(&mut b, 0x0000_0000); // dwProductVersionLS 0.0
    push_u32(&mut b, 0x0000_003F); // dwFileFlagsMask
    push_u32(&mut b, 0x0000_0000); // dwFileFlags
    push_u32(&mut b, 0x0004_0004); // dwFileOS = VOS_NT_WINDOWS32
    push_u32(&mut b, 0x0000_0001); // dwFileType = VFT_APP
    push_u32(&mut b, 0x0000_0000); // dwFileSubtype
    push_u32(&mut b, 0x0000_0000); // dwFileDateMS
    push_u32(&mut b, 0x0000_0000); // dwFileDateLS
    pad4(&mut b);

    let st = build_string_table(
        "040904B0",
        &[
            ("CompanyName", "Marmota Open Source Project"),
            (
                "FileDescription",
                "Marmota Vehicle Tester - War Thunder UserMissions Utility",
            ),
            ("FileVersion", "2.0.0.0"),
            ("InternalName", "marmota_vehicle_tester"),
            (
                "LegalCopyright",
                "Copyright (C) 2026 Marmota Vehicle Tester Contributors (GNU GPLv3)",
            ),
            ("OriginalFilename", "MarmotaVehicleTester.exe"),
            ("ProductName", "Marmota Vehicle Tester"),
            ("ProductVersion", "2.0.0.0"),
        ],
    );
    let sfi = build_string_file_info(&st);
    b.extend_from_slice(&sfi);
    pad4(&mut b);
    let vfi = build_var_file_info();
    b.extend_from_slice(&vfi);

    let len = (b.len() as u16).to_le_bytes();
    b[0] = len[0];
    b[1] = len[1];
    b
}

fn tool_available(tool: &str, check_flag: &str) -> bool {
    Command::new(tool)
        .arg(check_flag)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn find_windows_rc_tool(target_arch: &str) -> Option<PathBuf> {
    if tool_available("rc.exe", "/?") || tool_available("rc", "/?") {
        return Some(PathBuf::from("rc"));
    }
    if tool_available("llvm-rc", "/?") {
        return Some(PathBuf::from("llvm-rc"));
    }

    let sdk_arch = match target_arch {
        "aarch64" => "arm64",
        "x86" => "x86",
        _ => "x64",
    };

    for root in [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ] {
        let kits_bin = Path::new(root);
        if let Ok(entries) = std::fs::read_dir(kits_bin) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|v| v.path()))
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            versions.reverse();
            for ver_dir in versions {
                let candidate = ver_dir.join(sdk_arch).join("rc.exe");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
