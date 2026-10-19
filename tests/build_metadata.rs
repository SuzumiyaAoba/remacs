#[path = "../build.rs"]
mod build_script;

#[test]
fn darwin_configuration_records_build_host_release_for_target_architecture() {
    let configure = build_script::system_configuration;
    assert_eq!(
        configure(
            "aarch64-apple-darwin",
            "aarch64-apple-darwin",
            Some("25.5.0\n")
        ),
        "aarch64-apple-darwin25.5.0"
    );
    assert_eq!(
        configure(
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            Some("24.6.0")
        ),
        "x86_64-apple-darwin24.6.0"
    );
}

#[test]
fn cross_builds_and_missing_or_invalid_releases_keep_the_target() {
    let configure = build_script::system_configuration;
    let target = "aarch64-apple-darwin";
    assert_eq!(
        configure(target, "x86_64-unknown-linux-gnu", Some("6.12.1")),
        target
    );
    assert_eq!(
        configure("x86_64-unknown-linux-gnu", target, Some("25.5.0")),
        "x86_64-unknown-linux-gnu"
    );
    for release in [
        None,
        Some(""),
        Some("25..5"),
        Some("Darwin 25.5.0"),
        Some("25.5.0\nother"),
    ] {
        assert_eq!(configure(target, target, release), target);
    }
}
