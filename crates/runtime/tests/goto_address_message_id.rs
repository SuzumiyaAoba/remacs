use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(progn
  (require 'cl-lib)
  (require 'goto-addr)
  (let ((goto-address-prog-mode nil))
    (with-temp-buffer
      (insert "Message-ID: <abc@example.org>\nFrom: sender@example.org\n\n")
      (goto-address-fontify)
      (let ((message-id-link nil)
            (sender-link nil))
        (dolist (overlay (overlays-in (point-min) (point-max)))
          (when (overlay-get overlay 'goto-address)
            (let ((text (buffer-substring-no-properties
                         (overlay-start overlay) (overlay-end overlay))))
              (cond ((equal text "abc@example.org") (setq message-id-link t))
                    ((equal text "sender@example.org") (setq sender-link t))))))
        (unless (and (not message-id-link) sender-link)
          (error "Unexpected email links: Message-ID=%S From=%S"
                 message-id-link sender-link)))
      (let ((composed nil))
        (cl-letf (((symbol-function 'compose-mail)
                   (lambda (address) (setq composed address)))
                  ((symbol-function 'browse-url-url-at-point)
                   (lambda (&rest _) nil)))
          (goto-char (point-max))
          (search-backward "abc" nil t)
          (condition-case nil (goto-address-at-point) (error nil)))
        (when composed
          (error "Message-ID triggered compose-mail: %S" composed)))
      (let ((composed nil))
        (cl-letf (((symbol-function 'compose-mail)
                   (lambda (address) (setq composed address))))
          (goto-char (point-max))
          (search-backward "sender" nil t)
          (goto-address-at-point))
        (unless (equal composed "sender@example.org")
          (error "Ordinary email link changed: %S" composed)))
      t)))"#;

#[test]
fn message_id_header_is_not_treated_as_an_email_link() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.eval_str(CASES).unwrap_or_else(|flow| match flow {
                Flow::Signal(signal) => panic!(
                    "{} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                other => panic!("{other:?}"),
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
