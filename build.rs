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
/// Backslashes are doubled: resource scripts use C-style string
/// escapes, and a single-backslash Windows path is misread (the
/// Microsoft compiler rejected the script outright in the v0.9.0
/// release build, which then shipped without the icon).
fn write_rc(dir: &str) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let rc = out_dir.join("app_icon.rc");
    let ico = Path::new(dir).join("assets").join("app.ico");
    let path = ico.display().to_string().replace('\\', "\\\\");
    let body = format!("1 ICON \"{path}\"\n");
    if let Err(e) = std::fs::write(&rc, body) {
        println!("cargo:warning=could not write the icon resource script: {e}");
    }
    rc
}

/// The tail of a failed tool's output, flattened for a cargo warning.
fn tool_error(out: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.is_empty() {
        "(the tool printed no error)".to_string()
    } else {
        flat.chars().take(300).collect()
    }
}

fn embed_icon_gnu(dir: &str, target: &str) {
    let rc = write_rc(dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("app_icon.o");
    // x86_64-pc-windows-gnu -> x86_64-w64-mingw32-windres.
    let prefixed = target.replace("-pc-windows-gnu", "-w64-mingw32");
    let candidates = [format!("{prefixed}-windres"), "windres".to_string()];
    let mut last_err = "no candidate could be started".to_string();
    for cand in &candidates {
        match std::process::Command::new(cand)
            .arg(&rc)
            .args(["-O", "coff", "-o"])
            .arg(&out)
            .output()
        {
            Ok(o) if o.status.success() => {
                println!("cargo:rustc-link-arg-bins={}", out.display());
                return;
            }
            Ok(o) => last_err = tool_error(&o),
            Err(e) => last_err = e.to_string(),
        }
    }
    println!("cargo:warning=windres failed ({last_err}); building without the embedded app icon");
}

fn embed_icon_msvc(dir: &str) {
    let rc = write_rc(dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("app_icon.res");
    let Some(rc_exe) = find_rc_exe() else {
        println!("cargo:warning=rc.exe not found; building without the embedded app icon");
        return;
    };
    match std::process::Command::new(&rc_exe)
        .arg("/fo")
        .arg(&out)
        .arg(&rc)
        .output()
    {
        Ok(o) if o.status.success() => {
            println!("cargo:rustc-link-arg-bins={}", out.display());
        }
        Ok(o) => {
            println!(
                "cargo:warning=rc.exe failed ({}); building without the embedded app icon",
                tool_error(&o)
            );
        }
        Err(e) => {
            println!("cargo:warning=rc.exe could not be started ({e}); building without the embedded app icon");
        }
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
