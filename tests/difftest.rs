//! Differential sweep vs GNU Emacs (tools/difftest.py + tests/diff/driver.el).
//! Gated on DIFFTEST=1 and a `emacs` binary on PATH; otherwise skipped.
//! Full run takes ~10-15 min (remacs pays one boot per call batch).
#[test]
#[ignore = "long differential run: cargo test --test difftest -- --ignored"]
fn differential_vs_gnu() {
    if std::env::var("DIFFTEST").is_err() {
        eprintln!("skipped: set DIFFTEST=1");
        return;
    }
    if std::process::Command::new("emacs")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipped: no GNU emacs on PATH");
        return;
    }
    let remacs = option_env!("CARGO_BIN_EXE_remacs")
        .map(str::to_string)
        .unwrap_or_else(|| "target/debug/remacs".into());
    let status = std::process::Command::new("python3")
        .args([
            "tools/difftest.py",
            "--timeout",
            "1800",
            "--remacs",
            &remacs,
        ])
        .status()
        .expect("difftest.py failed to spawn");
    assert!(
        status.success(),
        "differential run found divergences (see tests/diff/report.txt)"
    );
}
