//! Shared dynamic-library loading helpers for optional C libraries
//! (libgnutls, libdbus-1, liblcms2, ...).
//!
//! Search order, mirroring GNU's delayed-load behavior:
//!   1. the `env_var` environment variable, when set (absolute path),
//!   2. each candidate name/path as-is (dyld resolves bare sonames via
//!      the usual search paths — Homebrew/MacPorts/system),
//!   3. a `/nix/store` scan for `*-<pkg>*/lib/<basename>' — nixpkgs
//!      libraries are not on the default search path.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{LazyLock, Mutex};

/// Try to load a library. `names` are tried literally first (absolute
/// paths or bare sonames like "libgnutls.dylib"), then
/// `/nix/store/*-<nix_pkg_prefix>-*/lib/<nix_basename>'.
pub fn open_library(
    env_var: &str,
    names: &[&str],
    nix_pkg_prefix: &str,
    nix_basename: &str,
) -> Option<libloading::Library> {
    let _dbg = std::env::var_os("REMACS_BOOT_PROF").is_some();
    if let Ok(p) = std::env::var(env_var) {
        if !p.is_empty() {
            if let Ok(l) = unsafe { libloading::Library::new(&p) } {
                return Some(l);
            }
        }
    }
    for n in names {
        if let Ok(l) = unsafe { libloading::Library::new(n) } {
            return Some(l);
        }
    }
    let t = _dbg.then(std::time::Instant::now);
    let found = nix_store_lib(nix_pkg_prefix, nix_basename);
    if let Some(t) = t {
        eprintln!(
            "[nix-scan {}] {}ms found={}",
            nix_pkg_prefix,
            t.elapsed().as_millis(),
            found.is_some()
        );
    }
    if let Some(p) = found {
        if let Ok(l) = unsafe { libloading::Library::new(&p) } {
            return Some(l);
        }
    }
    None
}

/// Scan `/nix/store' for the first `*-<prefix>-*/lib/<basename>' that
/// exists.  Deterministic: sorted candidate iteration.
fn nix_store_lib(prefix: &str, basename: &str) -> Option<String> {
    if basename.is_empty() {
        return None;
    }
    // One `read_dir' over /nix/store amortized across every optional
    // library: keep the raw `OsString' names (no per-entry `String'
    // alloc, no global sort — the old code spent ~650 ms doing both)
    // and derive each prefix's short sorted candidate list lazily.
    static ENTRIES: LazyLock<Vec<OsString>> = LazyLock::new(|| {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir("/nix/store") {
            for e in rd.flatten() {
                v.push(e.file_name());
            }
        }
        v
    });
    static CANDIDATES: LazyLock<Mutex<HashMap<String, Vec<OsString>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let needle = format!("-{}", prefix);
    let cands = {
        let mut m = CANDIDATES.lock().unwrap_or_else(|e| e.into_inner());
        match m.get(&needle) {
            Some(v) => v.clone(),
            None => {
                let mut v: Vec<OsString> = ENTRIES
                    .iter()
                    .filter(|n| n.to_string_lossy().contains(&needle))
                    .cloned()
                    .collect();
                v.sort();
                m.insert(needle.clone(), v.clone());
                v
            }
        }
    };
    for e in cands {
        let cand = format!("/nix/store/{}/lib/{}", e.to_string_lossy(), basename);
        if std::path::Path::new(&cand).exists() {
            return Some(cand);
        }
    }
    None
}

/// Resolve a symbol from a library into an arbitrary function pointer
/// type.  `unsafe` because the caller asserts the signature.
pub unsafe fn sym<T: Copy>(lib: &libloading::Library, name: &[u8]) -> Option<T> {
    let s: libloading::Symbol<T> = unsafe { lib.get(name) }.ok()?;
    Some(*s)
}
