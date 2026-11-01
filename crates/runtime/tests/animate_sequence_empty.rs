use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r###"(progn
  (unless (and (fboundp 'animate-sequence)
               (boundp 'animation-buffer-name))
    (error "animate.el source was not loaded"))
  (defvar remacs-animate-2710-sit-calls nil)
  (defvar remacs-animate-2710-original-sit-for nil)
  (defun remacs-animate-2710-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-animate-2710-sit-for-spy (&rest args)
    (push (car args) remacs-animate-2710-sit-calls)
    t)
  (defun remacs-animate-2710-check-origin (buffer window)
    (remacs-animate-2710-assert
     (and (eq (current-buffer) buffer)
          (eq (selected-window) window)
          (eq (window-buffer window) buffer))
     "An empty sequence changed current buffer or selected window"))
  (defun remacs-animate-2710-check-preserved (buffer text modified label)
    (remacs-animate-2710-assert
     (and (equal (with-current-buffer buffer (buffer-string)) text)
          (eq (with-current-buffer buffer (buffer-modified-p)) modified))
     (format "Empty sequence modified %s" label)))
  (defun remacs-animate-2710-check-delays (expected-count)
    (let ((delays (nreverse (copy-sequence remacs-animate-2710-sit-calls)))
          (all-zero t))
      (dolist (delay delays)
        (unless (= delay 0) (setq all-zero nil)))
      (remacs-animate-2710-assert
       (and (= (length delays) expected-count) all-zero)
       (format "Unexpected animation delays: %S" delays))))

  (setq remacs-animate-2710-original-sit-for (symbol-function 'sit-for))
  (fset 'sit-for #'remacs-animate-2710-sit-for-spy)
  (unwind-protect
      (progn
        (let* ((origin (get-buffer-create "*animate-2710-origin*"))
               (specified (get-buffer-create "*animate-2710-specified*"))
               (specified-name (buffer-name specified))
               (missing-name "*animate-2710-missing*"))
          (let ((old-missing (get-buffer missing-name)))
            (when old-missing
              (with-current-buffer old-missing (set-buffer-modified-p nil))
              (kill-buffer old-missing)))
          (with-current-buffer origin
            (erase-buffer)
            (insert "origin text")
            (set-buffer-modified-p nil))
          (with-current-buffer specified
            (erase-buffer)
            (insert "preserved specified animation")
            (set-buffer-modified-p t))
          (let ((legacy (get-buffer-create "*Animation*")))
            (with-current-buffer legacy
              (erase-buffer)
              (insert "preserved default animation")
              (set-buffer-modified-p t))
            (switch-to-buffer origin)
            (let* ((origin-window (selected-window))
                   (origin-text (buffer-string))
                   (origin-modified (buffer-modified-p))
                   (specified-text (with-current-buffer specified (buffer-string)))
                   (specified-modified
                    (with-current-buffer specified (buffer-modified-p)))
                   (legacy-text (with-current-buffer legacy (buffer-string)))
                   (legacy-modified (with-current-buffer legacy
                                      (buffer-modified-p))))
              ;; Empty LIST-OF-STRINGS is a no-op for an existing named target.
              (setq remacs-animate-2710-sit-calls nil)
              (let ((animation-buffer-name specified-name))
                (animate-sequence nil 1))
              (remacs-animate-2710-check-origin origin origin-window)
              (remacs-animate-2710-check-preserved
               origin origin-text origin-modified "origin buffer")
              (remacs-animate-2710-check-preserved
               specified specified-text specified-modified "named target")
              (remacs-animate-2710-check-preserved
               legacy legacy-text legacy-modified "default target")
              (remacs-animate-2710-check-delays 0)

              ;; An empty sequence does not create an absent named target.
              (setq remacs-animate-2710-sit-calls nil)
              (let ((animation-buffer-name missing-name))
                (animate-sequence nil 1))
              (remacs-animate-2710-check-origin origin origin-window)
              (remacs-animate-2710-assert
               (null (get-buffer missing-name))
               "Empty sequence created its configured target buffer")
              (remacs-animate-2710-check-delays 0)

              ;; Nil animation-buffer-name retains the existing default-buffer
              ;; contract, but an empty sequence must not erase that buffer.
              (setq remacs-animate-2710-sit-calls nil)
              (let ((animation-buffer-name nil))
                (animate-sequence nil 1))
              (remacs-animate-2710-check-origin origin origin-window)
              (remacs-animate-2710-check-preserved
               legacy legacy-text legacy-modified "default animation buffer")
              (remacs-animate-2710-check-delays 0))

            ;; With nil name, a nonempty sequence still uses *Animation*.
            (with-current-buffer legacy (set-buffer-modified-p nil))
            (kill-buffer legacy)
            (remacs-animate-2710-assert
             (null (get-buffer "*Animation*"))
             "Test setup failed to remove the default animation buffer")
            ;; Empty input with nil name must leave the absent default absent.
            (setq remacs-animate-2710-sit-calls nil)
            (let ((animation-buffer-name nil)
                  (origin-buffer (current-buffer))
                  (origin-window (selected-window)))
              (animate-sequence nil 1)
              (remacs-animate-2710-check-origin origin-buffer origin-window)
              (remacs-animate-2710-assert
               (null (get-buffer "*Animation*"))
               "Empty default animation created *Animation*")
              (remacs-animate-2710-check-delays 0))
            (setq remacs-animate-2710-sit-calls nil)
            (let ((animation-buffer-name nil)
                  (animate-n-steps 1)
                  (animate-total-added-delay 0))
              (animate-sequence '("Y") 0))
            (let ((default (get-buffer "*Animation*")))
              (remacs-animate-2710-assert
               (and default
                    (eq (current-buffer) default)
                    (eq (window-buffer (selected-window)) default))
               "Nonempty sequence with nil name did not select *Animation*" )
              (remacs-animate-2710-assert
               (with-current-buffer default
                 (save-excursion
                   (goto-char (point-min))
                   (search-forward "Y" nil t)))
               "Default animation did not display its final frame"))
            (remacs-animate-2710-check-delays 3)

            ;; A nonempty explicit name still creates, selects, erases, and
            ;; animates its configured destination.
            (with-current-buffer specified
              (erase-buffer)
              (insert "old target text")
              (set-buffer-modified-p nil))
            (switch-to-buffer origin)
            (setq remacs-animate-2710-sit-calls nil)
            (let ((animation-buffer-name specified-name)
                  (animate-n-steps 1)
                  (animate-total-added-delay 0))
              (animate-sequence '("X") 0))
            (remacs-animate-2710-assert
             (and (eq (current-buffer) specified)
                  (eq (window-buffer (selected-window)) specified))
             "Nonempty named sequence did not select its target")
            (remacs-animate-2710-assert
             (with-current-buffer specified
               (and (save-excursion
                      (goto-char (point-min))
                      (search-forward "X" nil t))
                    (not (string-match-p "old target text" (buffer-string)))))
             "Nonempty named sequence did not erase and display its frame")
            (remacs-animate-2710-check-delays 3)))
        t)
    (fset 'sit-for remacs-animate-2710-original-sit-for))
  t)"###;

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

#[test]
fn animate_sequence_nil_list_is_noop_and_nonempty_paths_stay_intact_2710() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/animate.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
