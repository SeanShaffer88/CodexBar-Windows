//! API-key storage. Keys are encrypted with Windows DPAPI
//! (`CryptProtectData`, current-user scope) and stored base64-encoded in
//! `data/secrets.json`. Plain-text keys are never written to disk.
//! On non-Windows builds DPAPI does not exist, so storing is refused with a
//! clear message (reading returns "no key").

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
struct SecretsFile {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openrouter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deepseek: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    moonshot: Option<String>,
}

/// Providers that take an API key stored here.
pub const KEY_PROVIDERS: [&str; 3] = ["openrouter", "deepseek", "moonshot"];

fn slot<'a>(f: &'a SecretsFile, provider: &str) -> Option<&'a String> {
    match provider {
        "openrouter" => f.openrouter.as_ref(),
        "deepseek" => f.deepseek.as_ref(),
        "moonshot" => f.moonshot.as_ref(),
        _ => None,
    }
}

fn set_slot(f: &mut SecretsFile, provider: &str, value: Option<String>) -> Result<(), String> {
    match provider {
        "openrouter" => f.openrouter = value,
        "deepseek" => f.deepseek = value,
        "moonshot" => f.moonshot = value,
        _ => return Err(format!("unknown key provider: {provider}")),
    }
    Ok(())
}

/// Store an API key for `provider`, DPAPI-encrypted.
pub fn set_key(dir: &Path, provider: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("the key is empty".to_string());
    }
    let blob = protect(key.as_bytes())?;
    let mut f = load_file(dir);
    f.schema_version = 1;
    set_slot(&mut f, provider, Some(base64_encode(&blob)))?;
    save_file(dir, &f)
}

/// Read the API key for `provider`, or None when none is stored.
pub fn get_key(dir: &Path, provider: &str) -> Option<String> {
    let enc = slot(&load_file(dir), provider)?.clone();
    let blob = base64_decode(&enc)?;
    let plain = unprotect(&blob).ok()?;
    String::from_utf8(plain).ok().map(|s| s.trim().to_string())
}

/// Remove the stored API key for `provider`.
pub fn clear_key(dir: &Path, provider: &str) -> Result<(), String> {
    let mut f = load_file(dir);
    f.schema_version = 1;
    set_slot(&mut f, provider, None)?;
    save_file(dir, &f)
}

fn secrets_path(dir: &Path) -> PathBuf {
    dir.join("secrets.json")
}

fn load_file(dir: &Path) -> SecretsFile {
    std::fs::read_to_string(secrets_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_file(dir: &Path, f: &SecretsFile) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create data folder: {e}"))?;
    let tmp = dir.join("secrets.json.tmp");
    let text = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write secrets: {e}"))?;
    std::fs::rename(&tmp, secrets_path(dir)).map_err(|e| format!("cannot replace secrets: {e}"))
}

/// Read the OpenRouter API key, or None when none is stored.
pub fn get_openrouter_key(dir: &Path) -> Option<String> {
    get_key(dir, "openrouter")
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::*;
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        if ok == 0 {
            return Err("Windows could not encrypt the key (DPAPI failed)".to_string());
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(v)
    }
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::*;
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        if ok == 0 {
            return Err("Windows could not decrypt the stored key (DPAPI failed)".to_string());
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(v)
    }
}

#[cfg(not(windows))]
fn protect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("API keys can only be stored on Windows (they are protected with DPAPI)".to_string())
}

#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("API keys can only be read on Windows (they are protected with DPAPI)".to_string())
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut vals: Vec<u32> = Vec::new();
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|&x| x == c)? as u32;
        vals.push(v);
    }
    let mut out = Vec::new();
    for chunk in vals.chunks(4) {
        let mut n: u32 = 0;
        for (i, v) in chunk.iter().enumerate() {
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for raw in [&b""[..], b"a", b"ab", b"abc", b"hello world, dpapi!"] {
            assert_eq!(base64_decode(&base64_encode(raw)).unwrap(), raw.to_vec());
        }
    }

    #[test]
    fn empty_key_refused() {
        let dir = std::env::temp_dir().join("cbw-sec-nonexistent");
        assert!(set_key(&dir, "openrouter", "  ").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let dir = std::env::temp_dir().join(format!("cbw-sec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        set_key(&dir, "openrouter", "sk-or-test-123").unwrap();
        let raw = std::fs::read_to_string(secrets_path(&dir)).unwrap();
        assert!(
            !raw.contains("sk-or-test-123"),
            "key must never be stored in plain text"
        );
        assert_eq!(get_openrouter_key(&dir).as_deref(), Some("sk-or-test-123"));
        clear_key(&dir, "openrouter").unwrap();
        assert_eq!(get_openrouter_key(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
