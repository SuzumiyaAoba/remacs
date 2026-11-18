use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn gnutls_hash_applies_string_and_buffer_coding_options() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            if interp.eval_str("(gnutls-available-p)").unwrap().is_nil() {
                return;
            }
            let result = interp.eval_str(
                    "(progn\
                       (defun qa-gnutls-input-hex (data)\
                         (mapconcat (lambda (byte) (format \"%02x\" byte))\
                                    (string-to-list data) \"\"))\
                       (defun qa-gnutls-safe-coding (_start _end _coding _default)\
                         'iso-latin-1-unix)\
                       (list\
                         (qa-gnutls-input-hex\
                          (gnutls-hash-digest 'SHA256\
                            (list \"é\" nil nil 'iso-latin-1-unix)))\
                         (with-temp-buffer\
                           (insert \"é\")\
                           (qa-gnutls-input-hex\
                            (gnutls-hash-digest 'SHA256\
                              (list (current-buffer) nil nil 'iso-latin-1-unix))))\
                         (let ((coding-system-for-write nil)\
                               (select-safe-coding-system-function\
                                'qa-gnutls-safe-coding))\
                           (with-temp-buffer\
                             (setq-local buffer-file-coding-system nil)\
                             (insert \"é\")\
                             (qa-gnutls-input-hex\
                              (gnutls-hash-digest 'SHA256\
                                (list (current-buffer))))))\
                         (string=\
                          (gnutls-hash-digest 'SHA256\
                            (list \"é\" nil nil 'qa-gnutls-invalid-coding t))\
                          (gnutls-hash-digest 'SHA256\
                            (list \"é\" nil nil 'raw-text)))\
                         (condition-case nil\
                             (gnutls-hash-digest 'SHA256\
                               (list \"é\" nil nil 'qa-gnutls-invalid-coding))\
                           (coding-system-error t))))",
                );
            let result = match result {
                Ok(result) => result,
                Err(Flow::Signal(signal)) => panic!(
                    "GnuTLS input coding signaled: {}",
                    interp.prin1_to_string(&signal.condition)
                ),
                Err(flow) => panic!("GnuTLS input coding failed: {flow:?}"),
            };
            let expected_digest =
                "de2e331d891ae267a7009cb45b4e8830f170e0c937288ea2731a1941c7a53b0d";
            assert_eq!(
                interp.prin1_to_string(&result),
                format!(
                    "(\"{expected_digest}\" \"{expected_digest}\" \"{expected_digest}\" t t)"
                )
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
