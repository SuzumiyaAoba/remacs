//! A failing initial write must not turn into a successful empty temp file.
#[path = "support/probe.rs"]
mod probe;

#[cfg(unix)]
#[test]
fn temporary_file_write_error_is_catchable() {
    probe::run("temporary_file_write_error_is_catchable", file!(), || {
        let mut interp = remacs::lisp::Interp::new();
        // The probe harness gives this test its own process. Limit only the
        // evaluation, restoring the limit and signal handler before reporting.
        struct WriteLimit {
            previous: libc::rlimit,
            signal: libc::sighandler_t,
        }
        impl WriteLimit {
            fn new() -> Self {
                let mut previous = std::mem::MaybeUninit::<libc::rlimit>::uninit();
                assert_eq!(
                    unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, previous.as_mut_ptr()) },
                    0
                );
                let previous = unsafe { previous.assume_init() };
                let signal = unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN) };
                assert_ne!(signal, libc::SIG_ERR);
                let guard = Self { previous, signal };
                let limited = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: guard.previous.rlim_max,
                };
                assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) }, 0);
                guard
            }
        }
        impl Drop for WriteLimit {
            fn drop(&mut self) {
                unsafe {
                    libc::setrlimit(libc::RLIMIT_FSIZE, &self.previous);
                    libc::signal(libc::SIGXFSZ, self.signal);
                }
            }
        }
        let limit = WriteLimit::new();
        let result = interp.eval_str(
            r#"
          (let ((root (make-temp-file "temp-write-error-" t)))
            (unwind-protect
                (mapcar
                 (lambda (internal)
                   (condition-case err
                       (progn
                         (if internal
                             (make-temp-file-internal
                              (expand-file-name "data-" root) nil "" "abc")
                           (make-temp-file
                            (expand-file-name "data-" root) nil nil "abc"))
                         'incorrect-success)
                     (file-error
                      (list (car err) (cadr err)
                            (stringp (car (last err)))))))
                 '(nil t))
              (delete-directory root t)))
        "#,
        );
        drop(limit);
        let value = result.unwrap_or_else(|flow| panic!("unexpected flow: {flow:?}"));
        assert_eq!(
            interp.prin1_to_string(&value),
            "((file-error \"Write error\" t) (file-error \"Write error\" t))"
        );
    });
}
