use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'icomplete)
  (defun remacs-test-icomplete-extra-hook ())
  (defun remacs-test-icomplete-state (fido enabled in-buffer)
    (let ((actual (list (and fido-mode t) (and icomplete-mode t)
                        (cl-count 'icomplete-minibuffer-setup minibuffer-setup-hook)
                        (cl-count 'icomplete--fido-mode-setup minibuffer-setup-hook)
                        (cl-count 'icomplete--in-region-setup completion-in-region-mode-hook)))
          (expected (list fido enabled (if enabled 1 0) (if fido 1 0)
                          (if (and enabled in-buffer) 1 0))))
      (unless (equal actual expected)
        (error "Completion mode state: expected %S, got %S" expected actual)))
    (unless (and (= (cl-count 'remacs-test-icomplete-extra-hook minibuffer-setup-hook) 1)
                 (= (cl-count 'remacs-test-icomplete-extra-hook completion-in-region-mode-hook) 1))
      (error "Completion modes removed or duplicated another hook")))
  (defun remacs-test-icomplete-setup ()
    (with-temp-buffer
      (cl-letf (((symbol-function 'icomplete-simple-completing-p) (lambda () t))
                ((symbol-function 'icomplete--field-string) (lambda () "prefix")))
        (funcall (car (memq 'icomplete-minibuffer-setup minibuffer-setup-hook))))
      (unless (and (local-variable-p 'icomplete--initial-input)
                   (equal icomplete--initial-input "prefix")
                   (memq 'icomplete-post-command-hook post-command-hook))
        (error "Registered Icomplete setup did not install completion display"))))
  (dolist (in-buffer '(nil t))
    (let ((icomplete-in-buffer in-buffer)
          (minibuffer-setup-hook (copy-sequence minibuffer-setup-hook))
          (completion-in-region-mode-hook (copy-sequence completion-in-region-mode-hook)))
      (add-hook 'minibuffer-setup-hook #'remacs-test-icomplete-extra-hook)
      (add-hook 'completion-in-region-mode-hook #'remacs-test-icomplete-extra-hook)
      (icomplete-mode -1)
      (fido-mode -1)
      (remacs-test-icomplete-state nil nil in-buffer)
      (fido-mode 1)
      (remacs-test-icomplete-state t t in-buffer)
      (fido-mode 1)
      (remacs-test-icomplete-state t t in-buffer)
      (fido-mode -1)
      (remacs-test-icomplete-state nil t in-buffer)
      (remacs-test-icomplete-setup)
      (fido-mode -1)
      (remacs-test-icomplete-state nil t in-buffer)
      (icomplete-mode -1)
      (fido-mode -1)
      (remacs-test-icomplete-state nil nil in-buffer)
      ;; Starting from ordinary Icomplete has the same coherent result.
      (icomplete-mode 1)
      (remacs-test-icomplete-state nil t in-buffer)
      (fido-mode 1)
      (remacs-test-icomplete-state t t in-buffer)
      (fido-mode -1)
      (remacs-test-icomplete-state nil t in-buffer)
      ;; An explicit switch to Icomplete must not recurse or duplicate hooks.
      (fido-mode 1)
      (icomplete-mode 1)
      (remacs-test-icomplete-state nil t in-buffer)
      (remacs-test-icomplete-setup)
      ;; Respect an explicit Icomplete disable before disabling Fido.
      (fido-mode 1)
      (icomplete-mode -1)
      (fido-mode -1)
      (remacs-test-icomplete-state nil nil in-buffer)))
  (let ((icomplete-in-buffer nil)
        (minibuffer-setup-hook '(remacs-test-icomplete-extra-hook))
        (completion-in-region-mode-hook '(remacs-test-icomplete-extra-hook))
        (icomplete-minibuffer-setup-hook nil))
    (fido-vertical-mode 1)
    (remacs-test-icomplete-state t t nil)
    (fido-mode -1)
    (remacs-test-icomplete-state nil t nil)
    (unless (and icomplete-vertical-mode
                 (memq 'icomplete--vertical-minibuffer-setup icomplete-minibuffer-setup-hook))
      (error "Fido disable discarded the Icomplete vertical setup"))
    (remacs-test-icomplete-setup)
    (fido-vertical-mode -1)
    (icomplete-mode -1)
    (fido-mode -1)
    (remacs-test-icomplete-state nil nil nil))
  t)
"#;

#[test]
fn disabling_fido_keeps_ordinary_icomplete_hooks_and_setup_active() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
