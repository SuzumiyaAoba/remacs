//! remacs — Emacs-compatible editor.
//!
//! Usage:
//!   remacs [options] [FILE...]
//!   --batch: batch mode (no TUI)
//!   -Q / --no-init: skip initialization files
//!   --eval EXPR / --execute EXPR: evaluate Lisp
//!   --load FILE / -l FILE: load a Lisp file
//!   --script FILE: batch + load
//!   --version

use remacs::cli;

use remacs::lisp::Interp;

const HELP: &str = r#"Usage: remacs [OPTION...] [FILE...]
An Emacs-compatible editor.

Startup options:
  --help, -help                 Display this help and exit
  --version                    Display the version and exit
  --batch                      Run without an interactive editor
  -nw, --no-window-system       Use the terminal UI instead of the GUI
  -Q, --quick                   Skip init files and X resources
  -q, --no-init-file            Skip user init files
  --no-x-resources              Ignore X resources
  --dump-file FILE             Restore a portable Remacs image

Actions are processed in command-line order:
  --eval EXPR, --execute EXPR   Evaluate the first Lisp object in EXPR
  -f, --funcall FUNCTION        Call FUNCTION
  -l, --load FILE               Load a Lisp file or library
  -L, --directory DIR           Add DIR to load-path; ':DIR' appends it
  --script FILE                Load FILE with lexical binding, then exit
  FILE                         Visit FILE before processing later actions
  --                           Treat all following arguments as filenames

GNU single-hyphen long aliases and --OPTION=VALUE are accepted where supported.
For diagnostics, --ieval EXPR evaluates forms and locates an evaluation error.
"#;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // GNU sets its `noninteractive' flag during main()'s argv scan —
    // before the interpreter is initialized — so init-time code like
    // `global-font-lock-mode''s `:init-value (not noninteractive)'
    // already sees the batch state.  Carry it to `Interp::new'.
    if cli::options(&args).any(|a| matches!(a, "--batch" | "-batch" | "--script")) {
        // SAFETY: set before Interp::new spawns work; the value is
        // only read during startup on this same thread anyway.
        unsafe { std::env::set_var("REMACS_NONINTERACTIVE", "1") };
    }
    // Deep Lisp recursion (eager macroexpansion of nested backquotes
    // in e.g. transient.el) overflows the default thread stack; GNU's
    // C eval frames are far smaller.  Batch and terminal sessions run
    // the interpreter on a worker with a generous stack.  The AppKit
    // GUI must stay on the main thread, so only the headless path is
    // moved; gui.rs enlarges its logic thread separately.
    let headless = cli::options(&args).any(|a| {
        matches!(
            a,
            "--batch"
                | "-batch"
                | "--script"
                | "--ieval"
                | "-nw"
                | "--no-window-system"
                | "--version"
                | "--help"
        )
    });
    if headless {
        run_headless(move || real_main(args));
        return;
    }
    real_main(args);
}

fn run_headless(run: impl FnOnce() + Send + 'static) {
    let child = std::thread::Builder::new()
        .name("remacs-main".into())
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
        .expect("failed to spawn interpreter thread");
    if let Err(panic) = child.join() {
        // Preserve the worker panic and Rust's failure exit status;
        // returning normally here used to report success to the caller.
        std::panic::resume_unwind(panic);
    }
}

#[cfg(test)]
mod headless_tests {
    use super::run_headless;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn waits_for_the_interpreter_worker_to_finish() {
        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        run_headless(move || worker_completed.store(true, Ordering::SeqCst));
        assert!(completed.load(Ordering::SeqCst));
    }

    #[test]
    fn worker_panic_propagates_after_unwinding_its_resources() {
        struct Cleanup(Arc<AtomicBool>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let cleaned = Arc::new(AtomicBool::new(false));
        let worker_cleaned = cleaned.clone();
        let panic = std::panic::catch_unwind(move || {
            run_headless(move || {
                let _cleanup = Cleanup(worker_cleaned);
                panic!("interpreter worker failure");
            })
        })
        .expect_err("worker panic was swallowed");
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"interpreter worker failure")
        );
        assert!(cleaned.load(Ordering::SeqCst));
    }
}

fn real_main(args: Vec<String>) {
    // GNU prints the version without booting the interpreter.
    if cli::options(&args).any(|a| a == "--version") {
        println!("remacs 0.1 (Emacs-compatible editor in Rust)");
        return;
    }
    // Version has GNU's priority; help still respects explicit/sidecar image
    // validation, but can skip Lisp bootstrap when there is no image.
    let help_requested = cli::options(&args).any(|a| a == "--help");
    let (args, dump_file) = match cli::dump_file(&args) {
        Ok(values) => values,
        Err(error) => {
            eprintln!("remacs: {error}");
            std::process::exit(255);
        }
    };
    let dump_file = dump_file.or_else(|| {
        std::env::current_exe()
            .ok()
            .map(|p| p.with_extension("pdmp"))
            .filter(|p| p.is_file())
            .map(|p| p.to_string_lossy().into_owned())
    });
    if help_requested && dump_file.is_none() {
        print!("{HELP}");
        return;
    }

    // GUI dispatch BEFORE `Interp::new': run_gui spawns its own
    // interpreter on the logic thread, so building one here would be
    // a thrown-away ~30s prelude — and any arg that forces batch or
    // the terminal front-end keeps this process headless.
    let batch_arg = help_requested
        || cli::options(&args).any(|a| {
            matches!(
                a,
                "--batch"
                    | "-batch"
                    | "--script"
                    | "--ieval"
            )
        });
    let no_window_arg = cli::options(&args).any(|a| matches!(a, "-nw" | "--no-window-system"));
    if !batch_arg && !no_window_arg {
        let files: Vec<String> = cli::arguments(&args)
            .filter_map(|arg| match arg {
                cli::Argument::File(name) => Some(name.to_string()),
                cli::Argument::Option { .. } => None,
            })
            .collect();
        return match remacs::gui::run_gui_with_dump(&files, dump_file.as_deref(), &args) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("remacs: gui error: {}", e);
                std::process::exit(1);
            }
        };
    }

    let mut i = match dump_file {
        None => Interp::new(),
        Some(path) => {
            match remacs::lisp::dump::load_with_arguments(std::path::Path::new(&path), Some(&args))
            {
                Ok(interpreter) => interpreter,
                Err(remacs::lisp::dump::LoadError::Exit(code)) => std::process::exit(code as i32),
                Err(error) => {
                    eprintln!("remacs: {error}");
                    std::process::exit(255);
                }
            }
        }
    };
    if help_requested {
        print!("{HELP}");
        return;
    }

    // `command-line-args'/`invocation-name'/`invocation-directory' are
    // seeded in Interp::new (GNU init_cmdargs placement).

    remacs::startup::run(&mut i, &args);

    // GUI sessions were dispatched above. Startup actions have completed;
    // the terminal session now enters its normal interactive command loop.
    match remacs::term::run_editor(&mut i) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("remacs: terminal error: {}", e);
            std::process::exit(1);
        }
    }
}
