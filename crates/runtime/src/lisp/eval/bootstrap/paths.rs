//! Discover GNU documentation and Info directories through installed Emacs layouts.

/// `<share>/emacs/<ver>/etc/` if it contains a DOC file.
fn doc_dir_under(emacs_share: &std::path::Path) -> Option<String> {
    let rd = std::fs::read_dir(emacs_share).ok()?;
    for e in rd.flatten() {
        let etc = e.path().join("etc");
        if etc.join("DOC").is_file() {
            return Some(format!("{}/", etc.display()));
        }
    }
    None
}

/// Find something under `<prefix>/share/` for the emacs binary at EXE.
/// CHECK is called with each candidate share dir. Wrapper scripts
/// (`exec /real/emacs`, as in Nixpkgs' emacsWithPackages) are unwrapped
/// up to DEPTH hops.
fn find_under_emacs_share(
    exe: &std::path::Path,
    depth: u8,
    check: &dyn Fn(&std::path::Path) -> Option<String>,
) -> Option<String> {
    let exe = std::fs::canonicalize(exe).ok()?;
    if let Some(prefix) = exe.parent().and_then(|p| p.parent()) {
        if let Some(hit) = check(&prefix.join("share")) {
            return Some(hit);
        }
        // Nixpkgs with-packages wrappers symlink share/* into the real
        // emacs package — follow any of them to the real prefix.
        if let Ok(rd) = std::fs::read_dir(prefix.join("share")) {
            for e in rd.flatten() {
                if let Ok(real) = std::fs::canonicalize(e.path()) {
                    if let Some(share) = real.parent() {
                        if let Some(hit) = check(share) {
                            return Some(hit);
                        }
                    }
                }
            }
        }
    }
    if depth < 5 {
        if let Ok(src) = std::fs::read_to_string(&exe) {
            for line in src.lines() {
                let Some(rest) = line.trim_start().strip_prefix("exec ") else {
                    continue;
                };
                let Some(target) = rest.split_whitespace().next() else {
                    continue;
                };
                let cand = std::path::PathBuf::from(target);
                if cand.file_name().map(|n| n == "emacs").unwrap_or(false) {
                    if let Some(hit) = find_under_emacs_share(&cand, depth + 1, check) {
                        return Some(hit);
                    }
                }
            }
        }
    }
    None
}

/// Find `<prefix>/share/emacs/<ver>/etc/` (containing DOC) for the
/// emacs binary at EXE.
pub(super) fn doc_dir_of_emacs(exe: &std::path::Path, depth: u8) -> Option<String> {
    find_under_emacs_share(exe, depth, &|share| doc_dir_under(&share.join("emacs")))
}

/// Find `<prefix>/share/info/` for the emacs binary at EXE — GNU's
/// configure-time PATH_INFO.
pub(super) fn info_dir_of_emacs(exe: &std::path::Path, depth: u8) -> Option<String> {
    find_under_emacs_share(exe, depth, &|share| {
        let dir = share.join("info");
        dir.is_dir().then(|| format!("{}/", dir.display()))
    })
}

/// Scan PATH (plus conventional dirs) for an `emacs` binary and apply F.
pub(super) fn find_emacs_on_path(f: &dyn Fn(&std::path::Path) -> Option<String>) -> Option<String> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain(
            [
                "/run/current-system/sw/bin",
                "/usr/bin",
                "/usr/local/bin",
                "/opt/homebrew/bin",
            ]
            .into_iter()
            .map(std::path::PathBuf::from),
        )
        .find_map(|dir| f(&dir.join("emacs")))
}
