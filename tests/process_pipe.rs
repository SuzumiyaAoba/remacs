//! Kernel evidence for subprocess pipes and descriptor ownership (#200).
mod common;
use common::*;
use remacs::lisp::process::ProcIo;
use remacs::lisp::{Interp, Value};
use std::os::fd::{AsRawFd, RawFd};
use std::time::{Duration, Instant};

// Descriptor reuse must not race another test in this process.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|e| panic!("{source}: {e:?}"))
}

fn pipe_fds(process: &Value) -> (RawFd, RawFd) {
    let Value::Process(process) = process else {
        panic!("process")
    };
    let process = process.borrow();
    let ProcIo::Child {
        input_fd,
        output_fd,
        tty_fd,
        ..
    } = &process.io
    else {
        panic!("child")
    };
    assert!(tty_fd.is_none(), "pipe child must not allocate a terminal");
    let fds = (
        input_fd.as_ref().unwrap().as_raw_fd(),
        output_fd.as_ref().unwrap().as_raw_fd(),
    );
    for fd in [fds.0, fds.1] {
        assert_eq!(unsafe { libc::isatty(fd) }, 0);
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::fstat(fd, &mut stat) }, 0);
        assert_eq!(
            stat.st_mode & libc::S_IFMT,
            libc::S_IFIFO,
            "must use kernel pipes"
        );
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    fds
}

fn closed(fd: RawFd) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

#[test]
fn pipe_eof_keeps_output_readable_and_all_descriptors_close() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        let process = eval(
            &mut i,
            "(setq pipe200 (make-process :name \"pipe200\" \
            :command '(\"/bin/cat\") :connection-type 'pipe :coding 'no-conversion \
            :buffer (get-buffer-create \" *pipe200*\") :noquery t :sentinel #'ignore))",
        );
        let (input, output) = pipe_fds(&process);
        eval(
            &mut i,
            "(process-send-string pipe200 \"A\\4B\\r\\0\\n\") (process-send-eof pipe200)",
        );
        closed(input);
        assert!(unsafe { libc::fcntl(output, libc::F_GETFD) } >= 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        while eval(&mut i, "(process-live-p pipe200)").truthy() {
            assert!(Instant::now() < deadline, "cat did not receive EOF");
            eval(&mut i, "(accept-process-output pipe200 0.01)");
        }
        let text = eval(
            &mut i,
            "(with-current-buffer \" *pipe200*\" (buffer-string))",
        );
        let Value::Str(text) = text else {
            panic!("string")
        };
        assert_eq!(text.borrow().to_external_bytes(), b"A\x04B\r\0\n");
        closed(output);

        let process = eval(
            &mut i,
            "(setq pipe200 (make-process :name \"pipe200-delete\" \
            :command '(\"/bin/cat\") :connection-type 'pipe :noquery t :sentinel #'ignore))",
        );
        let (input, output) = pipe_fds(&process);
        eval(&mut i, "(delete-process pipe200)");
        closed(input);
        closed(output);
    });
}

fn open_fds() -> Vec<RawFd> {
    (0..256)
        .filter(|&fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0)
        .collect()
}

fn split_fds(process: &Value) -> [RawFd; 3] {
    let (input, output) = pipe_fds(process);
    let Value::Process(process) = process else {
        unreachable!()
    };
    let process = process.borrow();
    let ProcIo::Child {
        stderr: Some(stderr),
        ..
    } = &process.io
    else {
        panic!("stderr pipe")
    };
    [input, output, stderr.as_raw_fd()]
}

#[test]
fn separate_stderr_closes_on_exit_and_delete() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        let process = eval(
            &mut i,
            "(setq split200 (make-process :name \"split200\" \
            :command '(\"/bin/sh\" \"-c\" \"printf ready; printf err >&2\") \
            :connection-type 'pipe :buffer (get-buffer-create \" *split200-out*\") \
            :stderr (get-buffer-create \" *split200-err*\") :noquery t :sentinel #'ignore))",
        );
        let descriptors = split_fds(&process);
        let deadline = Instant::now() + Duration::from_secs(3);
        while eval(&mut i, "(process-live-p split200)").truthy() {
            assert!(Instant::now() < deadline, "child did not exit");
            eval(&mut i, "(accept-process-output split200 0.01)");
        }
        for (buffer, expected) in [(" *split200-out*", "ready"), (" *split200-err*", "err")] {
            let text = eval(
                &mut i,
                &format!("(with-current-buffer {buffer:?} (buffer-string))"),
            );
            let Value::Str(text) = text else {
                panic!("string")
            };
            assert_eq!(text.borrow().as_str(), expected);
        }
        for fd in descriptors {
            closed(fd);
        }

        let process = eval(
            &mut i,
            "(setq split200 (make-process :name \"split200-delete\" \
            :command '(\"/bin/cat\") :connection-type 'pipe \
            :stderr (get-buffer-create \" *split200-err*\") :noquery t :sentinel #'ignore))",
        );
        let descriptors = split_fds(&process);
        eval(&mut i, "(delete-process split200)");
        for fd in descriptors {
            closed(fd);
        }
    });
}

#[test]
fn failed_spawns_release_pipe_and_terminal_descriptors() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        // Initialize the platform's terminal machinery before the snapshot.
        eval(
            &mut i,
            "(delete-process (make-process :name \"warm200\" \
            :command '(\"/bin/cat\") :connection-type 'pty :noquery t :sentinel #'ignore))",
        );
        let before = open_fds();
        for mode in ["pipe", "pty", "(pipe . pty)", "(pty . pipe)"] {
            for _ in 0..4 {
                let result = eval(
                    &mut i,
                    &format!(
                        "(condition-case nil \
                      (make-process :name \"missing200\" :connection-type '{mode} \
                       :command '(\"/remacs-issue200/no-such-executable\")) \
                      (file-missing 'expected))"
                    ),
                );
                let expected = i.intern("expected");
                assert!(remacs::lisp::eq_values(&result, &Value::Sym(expected)));
                assert_eq!(open_fds(), before, "descriptors leaked for {mode}");
            }
        }
    });
}
