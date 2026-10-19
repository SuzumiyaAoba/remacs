//! Cycle regressions run out of process so a reintroduced infinite loop
//! fails the test instead of leaving the complete QA runner spinning.
mod common;

use common::{interp, on_big_stack};
use remacs::lisp::{Flow, Value};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_cases(program: &std::ffi::OsStr) {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut child = ChildGuard(
        Command::new(program)
            .args([
                "-Q",
                "--batch",
                "-l",
                &format!("{root}/tests/qa_framework.el"),
                "-l",
                &format!("{root}/tests/qa_list_cycles.el"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start cycle regression process"),
    );
    let mut stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    let output = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let (status, timed_out) = loop {
        if let Some(status) = child.0.try_wait().expect("poll cycle regression") {
            break (status, false);
        }
        if Instant::now() >= deadline {
            child.0.kill().expect("stop hung cycle regression");
            break (child.0.wait().unwrap(), true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = output.join().unwrap();
    let stderr = errors.join().unwrap();
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !timed_out,
        "cycle regression exceeded 30 seconds: {program:?}"
    );
    assert!(
        status.success(),
        "cycle regression failed: {program:?}\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("checks passed"),
        "missing QA summary: {stdout}"
    );
}

#[test]
fn circular_list_search_and_removal() {
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        run_cases(&gnu);
    }
    run_cases(std::ffi::OsStr::new(env!("CARGO_BIN_EXE_remacs")));
}

#[test]
fn searches_allow_proper_lists_beyond_the_old_scan_limit() {
    on_big_stack(|| {
        let (mut i, _) = interp();
        let item = Value::cons(Value::Int(0), Value::Int(0));
        let length = 500_005;
        let source = Value::list(vec![item; length]);
        let mut failures = Vec::new();
        for name in [
            "memq", "memql", "member", "assq", "assoc", "rassq", "rassoc", "delq", "delete",
            "remq", "remove",
        ] {
            let symbol = i.intern(name);
            let result = i.apply(&Value::Sym(symbol), vec![Value::Int(9), source.clone()]);
            match result {
                Ok(Value::Nil) if !["delq", "delete", "remq", "remove"].contains(&name) => {}
                Ok(result) if ["delq", "delete", "remq"].contains(&name) => {
                    if !same_cons(&result, &source) {
                        failures.push(name);
                    }
                }
                Ok(result) if name == "remove" => {
                    let mut count = 0;
                    let mut tail = result.clone();
                    while let Value::Cons(cell) = tail {
                        count += 1;
                        tail = cell.borrow().cdr.clone();
                        if count > length {
                            break;
                        }
                    }
                    if count != length || !tail.is_nil() || same_cons(&result, &source) {
                        failures.push(name);
                    }
                    // This is an independent copied spine.
                    if !same_cons(&result, &source) {
                        release_spine(result);
                    }
                }
                _ => failures.push(name),
            }
        }
        release_spine(source);
        assert!(
            failures.is_empty(),
            "proper-list scans failed: {failures:?}"
        );
    });
}

fn same_cons(one: &Value, two: &Value) -> bool {
    matches!((one,two),(Value::Cons(one),Value::Cons(two)) if Rc::ptr_eq(one,two))
}

// Release long fixtures iteratively so the test does not depend on the
// unrelated recursive Rc destructor's stack consumption.
fn release_spine(mut list: Value) {
    while let Value::Cons(cell) = list {
        list = std::mem::replace(&mut cell.borrow_mut().cdr, Value::Nil);
    }
}

#[test]
fn native_search_checks_pending_quit_without_preempting_a_hit() {
    on_big_stack(|| {
        let (mut i, _) = interp();
        let symbol = i.intern("memq");
        let Value::Subr(subr) = i.symbol_function(symbol) else {
            panic!("memq is not a native primitive")
        };
        let source = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        i.quit_flag = true;
        let hit = (subr.func)(&mut i, vec![Value::Int(1), source.clone()].into()).unwrap();
        assert!(same_cons(&hit, &source));
        assert!(i.quit_flag);
        let result = (subr.func)(&mut i, vec![Value::Int(9), source].into());
        assert!(matches!(result, Err(Flow::Quit)));
        assert!(!i.quit_flag);
    });
}
