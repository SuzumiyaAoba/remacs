//! Shared dynamic-library loading helpers for optional C libraries
//! (libgnutls, libdbus-1, liblcms2, ...).
//!
//! Search order, mirroring GNU's delayed-load behavior:
//!   1. the `env_var` environment variable, when set (absolute path),
//!   2. each candidate name/path as-is (dyld resolves bare sonames via
//!      the usual search paths — Homebrew/MacPorts/system),
//!   3. a `/nix/store` scan for `*-<pkg>*/lib/<basename>' — nixpkgs
//!      libraries are not on the default search path.

use std::sync::OnceLock;

/// Try to load a library. `names` are tried literally first (absolute
/// paths or bare sonames like "libgnutls.dylib"), then
/// `/nix/store/*-<nix_pkg_prefix>-*/lib/<nix_basename>'.
pub fn open_library(
    env_var: &str,
    names: &[&str],
    nix_pkg_prefix: &str,
    nix_basename: &str,
) -> Option<libloading::Library> {
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
    if let Some(p) = nix_store_lib(nix_pkg_prefix, nix_basename) {
        if let Ok(l) = unsafe { libloading::Library::new(&p) } {
            return Some(l);
        }
    }
    None
}

/// Scan `/nix/store' for the first `*-<prefix>-*/lib/<basename>' that
/// exists.  Deterministic: sorted directory iteration.
fn nix_store_lib(prefix: &str, basename: &str) -> Option<String> {
    if basename.is_empty() {
        return None;
    }
    static STORE: OnceLock<Vec<String>> = OnceLock::new();
    let entries = STORE.get_or_init(|| {
        let mut v: Vec<String> = Vec::new();
        if let Ok(rd) = std::fs::read_dir("/nix/store") {
            for e in rd.flatten() {
                v.push(e.file_name().to_string_lossy().into_owned());
            }
        }
        v.sort();
        v
    });
    for e in entries {
        // Match names like "kvn…-gnutls-3.8.13" for prefix "gnutls".
        if !e.contains(&format!("-{}", prefix)) {
            continue;
        }
        let cand = format!("/nix/store/{}/lib/{}", e, basename);
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
