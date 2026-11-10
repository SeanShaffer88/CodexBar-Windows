// Embed the application manifest (Per-Monitor v2 DPI awareness) into
// MSVC builds, and the application icon (assets/app.ico, resource
// id 1) into every Windows build. GNU/MinGW builds rely on the
// startup SetProcessDpiAwarenessContext call for DPI instead, which
// runs before any window exists. No external crates: the icon is
// compiled with the toolchain's own resource compiler (windres for
// GNU targets, rc.exe from the Windows SDK for MSVC targets); when
// neither is available the build still succeeds, just without the
// embedded icon, and says so in a warning.

use std::path::{Path, PathBuf};

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if !target.contains("windows") {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    if target.contains("msvc") {
        let manifest = Path::new(&dir).join("app.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        embed_icon_msvc(&dir);
    } else {
        embed_icon_gnu(&dir, &target);
    }
}

/// Write a one-line resource script naming the icon by absolute path,
/// so neither resource compiler has to resolve a relative path.
fn write_rc(dir: &str) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let rc = out_dir.join("app_icon.rc");
    let ico = Path::new(dir).join("assets").join("app.ico");
    let body = format!("1 ICON \"{}\"\n", ico.display());
    if let Err(e) = std::fs::write(&rc, body) {
        println!("cargo:warning=could not write the icon resource script: {e}");
    }
    rc
}

fn embed_icon_gnu(dir: &str, target: &str) {
    let rc = write_rc(dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("app_icon.o");
    // x86_64-pc-windows-gnu -> x86_64-w64-mingw32-windres.
    let prefixed = target.replace("-pc-windows-gnu", "-w64-mingw32");
    let candidates = [format!("{prefixed}-windres"), "windres".to_string()];
    for cand in &candidates {
        let ok = std::process::Command::new(cand)
            .arg(&rc)
            .args(["-O", "coff", "-o"])
            .arg(&out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            println!("cargo:rustc-link-arg-bins={}", out.display());
            return;
        }
    }
    println!("cargo:warning=windres not found; building without the embedded app icon");
}

fn embed_icon_msvc(dir: &str) {
    let rc = write_rc(dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("app_icon.res");
    let Some(rc_exe) = find_rc_exe() else {
        println!("cargo:warning=rc.exe not found; building without the embedded app icon");
        return;
    };
    let ok = std::process::Command::new(&rc_exe)
        .arg("/fo")
        .arg(&out)
        .arg(&rc)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", out.display());
    } else {
        println!("cargo:warning=rc.exe failed; building without the embedded app icon");
    }
}

/// Find the newest x64 rc.exe in the Windows 10/11 SDK, if installed.
fn find_rc_exe() -> Option<PathBuf> {
    let roots = [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ];
    let mut found: Vec<PathBuf> = Vec::new();
    for root in roots {
        if let Ok(rd) = std::fs::read_dir(root) {
            for entry in rd.flatten() {
                let p = entry.path().join("x64").join("rc.exe");
                if p.exists() {
                    found.push(p);
                }
            }
        }
    }
    found.sort();
    found.pop()
}
