/// Cargo omits the kernel release from its Darwin target triple. GNU's
/// config.guess includes it. Only use a Darwin build host's release for
/// Darwin targets; a cross build must not acquire another OS's version.
pub(crate) fn system_configuration(target: &str, host: &str, release: Option<&str>) -> String {
    if target.ends_with("-apple-darwin") && host.ends_with("-apple-darwin") {
        if let Some(release) = release.map(str::trim).filter(|release| {
            !release.is_empty()
                && release
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        }) {
            return format!("{target}{release}");
        }
    }
    target.to_owned()
}

#[cfg(not(test))]
pub(crate) fn source_root() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if manifest.ends_with("crates/runtime") {
        manifest.parent().unwrap().parent().unwrap().to_path_buf()
    } else {
        manifest
    }
}

#[cfg(not(test))]
pub(crate) fn main() {
    use std::env;
    use std::process::Command;

    let root = source_root();
    println!("cargo:rustc-env=REMACS_SOURCE_ROOT={}", root.display());
    println!("cargo:rerun-if-changed={}", root.join("build.rs").display());
    let target = env::var("TARGET").expect("Cargo TARGET");
    let host = env::var("HOST").expect("Cargo HOST");
    let release = if target.ends_with("-apple-darwin") && host.ends_with("-apple-darwin") {
        Command::new("uname")
            .arg("-r")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
    } else {
        None
    };
    println!(
        "cargo:rustc-env=REMACS_SYSTEM_CONFIGURATION={}",
        system_configuration(&target, &host, release.as_deref())
    );
}
