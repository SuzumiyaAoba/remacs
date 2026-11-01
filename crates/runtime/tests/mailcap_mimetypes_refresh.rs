use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'mailcap)
  (require 'cl-lib)
  (defvar remacs-test-mailcap-phase nil)
  (defun remacs-test-mime-write (file contents)
    (with-temp-file file (insert contents)))
  (let* ((directory (make-temp-file "remacs-mime-refresh-" t))
         (first (expand-file-name "first.types" directory))
         (second (expand-file-name "second.types" directory)))
    (unwind-protect
        (progn
          (setq remacs-test-mailcap-phase 'custom-defaults-and-cache)
          (let* ((custom (cons ".manual" "application/x-manual"))
                 (default (assoc ".txt" mailcap-mime-extensions))
                 (mailcap-mime-extensions (cons custom (copy-sequence mailcap-mime-extensions)))
                 (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-old foo txt\n")
            (unless (eq (mailcap-parse-mimetypes first t) t) (error "Initial parse did not succeed"))
            (unless (and (equal (mailcap-extension-to-mime "foo") "application/x-old")
                         (equal (mailcap-extension-to-mime ".txt") "application/x-old"))
              (error "File entries did not override the default mapping"))
            (remacs-test-mime-write first "application/x-new bar\n")
            (unless (and (null (mailcap-parse-mimetypes first))
                         (equal (mailcap-extension-to-mime "foo") "application/x-old")
                         (null (mailcap-extension-to-mime "bar")))
              (error "A non-forced call bypassed the parsed flag"))
            (mailcap-parse-mimetypes first t)
            (unless (and (null (mailcap-extension-to-mime "foo"))
                         (equal (mailcap-extension-to-mime "bar") "application/x-new")
                         (equal (mailcap-extension-to-mime "manual") "application/x-manual")
                         (eq (assoc ".txt" mailcap-mime-extensions) default)
                         (memq custom mailcap-mime-extensions))
              (error "Forced refresh retained a removed mapping or lost Lisp/default entries"))
            (let ((count (length mailcap-mime-extensions)))
              (mailcap-parse-mimetypes first t)
              (unless (= count (length mailcap-mime-extensions))
                (error "Repeated forced scans accumulated duplicates")))
            (remacs-test-mime-write first "")
            (mailcap-parse-mimetypes first t)
            (unless (and (null (mailcap-extension-to-mime "bar"))
                         (eq (assoc ".txt" mailcap-mime-extensions) default))
              (error "An empty file left stale mappings")))
          (setq remacs-test-mailcap-phase 'priority-removal-and-paths)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-first shared only-first\n")
            (remacs-test-mime-write second "application/x-second shared only-second\n")
            (mailcap-parse-mimetypes (list first second) t)
            (unless (equal (mailcap-extension-to-mime "shared") "application/x-first")
              (error "PATH priority changed"))
            (remacs-test-mime-write first "application/x-new new\n")
            (mailcap-parse-mimetypes (mapconcat #'identity (list first second) path-separator) t)
            (unless (and (equal (mailcap-extension-to-mime "shared") "application/x-second")
                         (null (mailcap-extension-to-mime "only-first"))
                         (equal (mailcap-extension-to-mime "only-second") "application/x-second"))
              (error "Removing a higher-priority mapping did not expose the lower one"))
            (delete-file second)
            (mailcap-parse-mimetypes (list first second) t)
            (unless (and (null (mailcap-extension-to-mime "shared"))
                         (null (mailcap-extension-to-mime "only-second")))
              (error "An absent file left old mappings"))
            (mailcap-parse-mimetypes "" t)
            (unless (null mailcap-mime-extensions) (error "An empty path retained file data")))
          (setq remacs-test-mailcap-phase 'changed-value-and-flag-reset)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-old same\n")
            (mailcap-parse-mimetypes first t)
            (let ((old (assoc ".same" mailcap-mime-extensions)))
              (remacs-test-mime-write first "application/x-new same\n")
              (setq mailcap-mimetypes-parsed-p nil)
              (mailcap-parse-mimetypes first)
              (unless (and (equal (mailcap-extension-to-mime "same") "application/x-new")
                           (= (length mailcap-mime-extensions) 1)
                           (not (memq old mailcap-mime-extensions)))
                (error "A flag reset did not replace the previous scan"))))
          (setq remacs-test-mailcap-phase 'equal-custom-and-direct-parser)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-old same\n")
            (mailcap-parse-mimetypes first t)
            (let* ((old (assoc ".same" mailcap-mime-extensions))
                   (custom (copy-tree old)))
              (push custom mailcap-mime-extensions)
              (remacs-test-mime-write first "application/x-new new\n")
              (remacs-test-mime-write second "application/x-direct direct\n")
              (mailcap-parse-mimetype-file second)
              (let ((direct (assoc ".direct" mailcap-mime-extensions)))
                (mailcap-parse-mimetypes first t)
                (unless (and (memq custom mailcap-mime-extensions)
                             (not (memq old mailcap-mime-extensions))
                             (memq direct mailcap-mime-extensions)
                             (equal (mailcap-extension-to-mime "same") "application/x-old")
                             (equal (mailcap-extension-to-mime "direct") "application/x-direct"))
                  (error "Equal custom entries or direct-parser additions were discarded")))))
          (setq remacs-test-mailcap-phase 'edited-parsed-pairs)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first
                                    "application/x-value value-only\napplication/x-key key-only\napplication/x-pair pair\n")
            (mailcap-parse-mimetypes first t)
            (let ((value-entry (assoc ".value-only" mailcap-mime-extensions))
                  (key-entry (assoc ".key-only" mailcap-mime-extensions))
                  (pair-entry (assoc ".pair" mailcap-mime-extensions)))
              (aset (cdr value-entry) (1- (length (cdr value-entry))) ?X)
              (aset (car key-entry) (1- (length (car key-entry))) ?z)
              (setcdr pair-entry "application/x-lisp")
              (let ((value (copy-sequence (cdr value-entry))) (key (copy-sequence (car key-entry))))
                (remacs-test-mime-write first "")
                (mailcap-parse-mimetypes first t)
                (unless (and (eq (assoc ".value-only" mailcap-mime-extensions) value-entry)
                             (equal (mailcap-extension-to-mime "value-only") value)
                             (eq (assoc key mailcap-mime-extensions) key-entry)
                             (null (mailcap-extension-to-mime "key-only"))
                             (equal (mailcap-extension-to-mime "pair") "application/x-lisp"))
                  (error "An in-place Lisp customization was treated as stale file data")))))
          (setq remacs-test-mailcap-phase 'dynamic-bindings-and-gc)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-outer outer\n")
            (mailcap-parse-mimetypes first t)
            (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
              (remacs-test-mime-write second "application/x-inner inner\n")
              (mailcap-parse-mimetypes second t)
              (garbage-collect)
              (unless (equal (mailcap-extension-to-mime "inner") "application/x-inner")
                (error "A live inner mapping was lost during GC")))
            (unless (equal (mailcap-extension-to-mime "outer") "application/x-outer")
              (error "Temporary bindings did not restore their outer table"))
            (remacs-test-mime-write first "")
            (mailcap-parse-mimetypes first t)
            (unless (null (mailcap-extension-to-mime "outer"))
              (error "An outer scan lost its ownership across a temporary binding/GC")))
          (setq remacs-test-mailcap-phase 'failed-read-atomicity)
          (let ((mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-old old\n")
            (mailcap-parse-mimetypes first t)
            (let ((before mailcap-mime-extensions))
              (remacs-test-mime-write second "application/x-new new\n")
              ;; Reverse PATH reads SECOND successfully before failing on a directory.
              (unless (condition-case nil
                          (progn (mailcap-parse-mimetypes (list directory second) t) nil)
                        (file-error t))
                (error "The failing-read fixture did not signal"))
              (unless (and (eq before mailcap-mime-extensions) mailcap-mimetypes-parsed-p
                           (equal (mailcap-extension-to-mime "old") "application/x-old")
                           (null (mailcap-extension-to-mime "new")))
                (error "A failed reparse published partial data"))
              (mailcap-parse-mimetypes second t)
              (unless (and (null (mailcap-extension-to-mime "old"))
                           (equal (mailcap-extension-to-mime "new") "application/x-new"))
                (error "A retry after a failed parse retained stale entries"))))
          (setq remacs-test-mailcap-phase 'environment-path)
          (let ((process-environment (copy-sequence process-environment))
                (mailcap-mime-extensions nil) (mailcap-mimetypes-parsed-p nil))
            (remacs-test-mime-write first "application/x-env env\n")
            (setenv "MIMETYPES" first)
            (mailcap-parse-mimetypes nil t)
            (unless (equal (mailcap-extension-to-mime "env") "application/x-env")
              (error "The MIMETYPES path was not used"))
            (remacs-test-mime-write first "")
            (mailcap-parse-mimetypes nil t)
            (unless (null (mailcap-extension-to-mime "env"))
              (error "Environment-path refresh retained deleted entries"))))
      (delete-directory directory t)))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-mailcap-phase");
            let phase = interp.symbol_value(symbol);
            panic!(
                "{} {} phase={}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase)
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn forced_mimetypes_scans_replace_only_their_own_mappings() {
    let mut interp = Interp::new();
    eprintln!("default mailcap MIME types refresh");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "mailcap.el" nil t)"#);
    eprintln!("source mailcap MIME types refresh");
    check(&mut interp, CASES);
}
