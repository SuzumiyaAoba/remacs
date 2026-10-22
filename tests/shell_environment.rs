//! Startup shell selection must use the child process's own environment.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

fn check(shell: Option<&str>, eshell: Option<&str>, form: &str, expected: &str) {
    let run = |program: &std::ffi::OsStr, source: bool| {
        let mut command = Command::new(program);
        command
            .args(["-Q", "--batch", "--eval", form])
            .env_remove("SHELL")
            .env_remove("ESHELL")
            .env_remove("REMACS_NO_ELC");
        if let Some(shell) = shell {
            command.env("SHELL", shell);
        }
        if let Some(eshell) = eshell {
            command.env("ESHELL", eshell);
        }
        if source {
            command.env("REMACS_NO_ELC", "1");
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{program:?} source={source}: {stderr}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{program:?} SHELL={shell:?} ESHELL={eshell:?} source={source}: {stderr}"
        );
    };
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        run(&gnu, false);
    }
    for source in [false, true] {
        run(std::ffi::OsStr::new(env!("CARGO_BIN_EXE_remacs")), source);
    }
}

#[test]
fn startup_uses_shell_and_ignores_eshell() {
    for (shell, eshell, expected) in [
        (None, None, "\"/bin/sh\""),
        (Some("/bin/zsh"), Some("/bin/sh"), "\"/bin/zsh\""),
        (None, Some("/bin/zsh"), "\"/bin/sh\""),
        (Some(""), None, "\"\""),
        (Some("/missing/shell"), None, "\"/missing/shell\""),
    ] {
        check(shell, eshell, "(prin1 shell-file-name)", expected);
    }
}

struct Shell(PathBuf);
impl Drop for Shell {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn selected_shell_runs_and_an_explicit_binding_overrides_it() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "remacs-shell-environment-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let fixture = Shell(root);
    let shell = fixture.0.join("selected shell");
    std::fs::write(
        &shell,
        b"#!/bin/sh\nprintf 'selected:'\nif [ \"$1\" = --selected-switch ]; then\n  printf 'switch:'\n  shift\n  set -- -c \"$@\"\nfi\nexec /bin/sh \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
    check(
        Some(shell.to_str().unwrap()),
        Some("/ignored/eshell"),
        r#"(prin1 (list
          (shell-command-to-string "printf audit-shell-ok")
          (let ((shell-file-name "/bin/sh"))
            (shell-command-to-string "printf audit-shell-ok"))
          (progn (setenv "SHELL" "/changed-after-startup")
                 (shell-command-to-string "printf audit-shell-ok"))
          (with-temp-buffer
            (shell-command "printf audit-shell-ok" (current-buffer))
            (buffer-string))
          (let ((shell-command-switch "--selected-switch"))
            (shell-command-to-string "printf audit-shell-ok"))))"#,
        r#"("selected:audit-shell-ok" "audit-shell-ok" "selected:audit-shell-ok" "selected:audit-shell-ok" "selected:switch:audit-shell-ok")"#,
    );
}
