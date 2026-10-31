use remacs_runtime::lisp::{Flow, Interp};

const PRIMITIVE_CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (defvar remacs-test-eww-disabled-phase nil)
  (setq remacs-test-eww-disabled-phase 'primitive-writable-fields)
  (defun remacs-test-ro-result (fn)
    (condition-case err (progn (funcall fn) 'ok) (error (car err))))
  (with-temp-buffer
    (insert "abcd")
    (put-text-property 2 4 'inhibit-read-only t)
    (put-text-property 2 4 'field-tag 'editable)
    (goto-char 3)
    (setq buffer-read-only t)
    (unless (equal (mapcar
                    (lambda (pos)
                      (list pos (remacs-test-ro-result
                                 (lambda () (barf-if-buffer-read-only pos)))))
                    '(nil -1 0 1 2 3 4 5 99))
                   '((nil ok) (-1 args-out-of-range) (0 args-out-of-range)
                     (1 buffer-read-only) (2 ok) (3 ok) (4 buffer-read-only)
                     (5 buffer-read-only) (99 args-out-of-range)))
      (error "barf-if-buffer-read-only ignored its POSITION/property"))
    (let ((last-command-event ?X)) (self-insert-command 2))
    (unless (and (equal (buffer-string) "abXXcd")
                 (get-text-property 3 'inhibit-read-only)
                 (get-text-property 4 'inhibit-read-only)
                 (eq (get-text-property 3 'field-tag) 'editable)
                 (eq (get-text-property 4 'field-tag) 'editable))
      (error "self-insert did not inherit writable field properties"))
    (delete-char -2)
    (unless (equal (buffer-string) "abcd") (error "Field rollback failed"))
    (goto-char 1)
    (unless (eq (remacs-test-ro-result
                 (lambda () (let ((last-command-event ?X)) (self-insert-command 1))))
                'buffer-read-only)
      (error "Text outside the writable field became editable"))
    (let ((marker (copy-marker 2)))
      (dolist (readonly '(nil t))
        (let ((buffer-read-only readonly))
          (unless (equal (condition-case err (barf-if-buffer-read-only marker)
                           (error (list (car err) (cadr err))))
                         '(wrong-type-argument fixnump))
            (error "POSITION marker was incorrectly accepted")))))
    (let ((buffer-read-only nil))
      (barf-if-buffer-read-only -1)
      (barf-if-buffer-read-only 99))
    (let ((inhibit-read-only t)) (barf-if-buffer-read-only 99))
    (unless (equal (condition-case err (barf-if-buffer-read-only 99) (error err))
                   '(args-out-of-range 99 99))
      (error "Conditional POSITION range validation changed")))
  (with-temp-buffer
    (insert "abcd")
    (put-text-property 2 4 'inhibit-read-only t)
    (goto-char 1)
    (setq buffer-read-only t)
    (delete-region 2 4)
    (unless (equal (buffer-string) "ad")
      (error "Range deletion checked point instead of range start")))
  (with-temp-buffer
    (insert "abcd")
    (put-text-property 2 4 'inhibit-read-only t)
    (goto-char 2)
    (setq buffer-read-only t)
    (let* ((calls 0)
          (before-change-functions (list (lambda (&rest _) (setq calls (1+ calls))))))
      (unless (and (eq (remacs-test-ro-result (lambda () (delete-region 2 5))) 'buffer-read-only)
                   (equal (buffer-string) "abcd") (= calls 0))
        (error "Mixed writable/readonly deletion was not atomic"))))
  (with-temp-buffer
    (insert "abcd")
    (put-text-property 1 5 'inhibit-read-only t)
    (goto-char (point-max))
    (setq buffer-read-only t)
    (erase-buffer)
    (unless (= (buffer-size) 0) (error "Erase checked point instead of first character")))
  t)
"##;

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defvar remacs-test-eww-disabled-phase nil)
  (defvar remacs-test-eww-disabled-setup nil)
  (defun remacs-test-eww-field (id)
    (save-excursion
      (goto-char (point-min))
      (let (found)
        (while (and (< (point) (point-max)) (not found))
          (let ((field (get-text-property (point) 'eww-form)))
            (when (equal (dom-attr (plist-get field :dom) 'id) id)
              (setq found (cons (point) field))))
          (unless found (forward-char)))
        (or found (error "No rendered control %S" id)))))
  (defun remacs-test-eww-disabled-query (method id)
    (let (result)
      (goto-char (car (remacs-test-eww-field id)))
      (cl-letf (((symbol-function 'eww-browse-url)
                 (lambda (url &rest _) (setq result (list url url-request-data)))))
        (eww-submit))
      (unless result (error "Expected form submission from %S" id))
      (if (equal method "post") (nth 1 result)
        (cdr (url-path-and-query (url-generic-parse-url (car result)))))))
  (defun remacs-test-eww-edit-blocked (position)
    (goto-char position)
    (condition-case nil
        (progn (let ((last-command-event ?X)) (self-insert-command 1)) nil)
      ((buffer-read-only text-read-only) t)))
  (defun remacs-test-eww-disabled
      (method children expected disabled-ids readonly-ids &optional table outside submit-id callback enctype)
    (with-temp-buffer
      (setq remacs-test-eww-disabled-phase (list 'mode method table outside))
      (eww-mode)
      (let* ((eww-data (list :url "https://example.test/page" :title "Test"))
             (shr-use-fonts nil)
             (shr-width 100)
             (submit (dom-node 'button '((id . "_submit")) "Send"))
             (nodes (append children (list submit)))
             (form (append (list 'form (list (cons 'method method)
                                            (cons 'action "https://example.test/search")
                                            (cons 'enctype enctype)))
                           (if table
                               (list (dom-node 'table nil
                                               (append (list 'tr nil)
                                                       (mapcar (lambda (node) (dom-node 'td nil node))
                                                               nodes))))
                             nodes)))
             (body (if outside
                       (dom-node 'fieldset '((disabled . ""))
                                 (if (eq outside 'table)
                                     (dom-node 'table nil (dom-node 'tr nil (dom-node 'td nil form)))
                                   form))
                     form))
             (document (dom-node 'html nil (dom-node 'body nil body)))
             (expected-query (if (and (consp expected) (not (functionp expected)))
                                 (car expected) expected)))
        (setq remacs-test-eww-disabled-phase (list 'render method table outside))
        (eww-display-document (eww-document-base "https://example.test/page" document))
        (when (bound-and-true-p remacs-test-eww-disabled-setup)
          (funcall remacs-test-eww-disabled-setup))
        (when (member "upload" disabled-ids)
          (plist-put (cdr (remacs-test-eww-field "upload")) :filename
                     "/private/tmp/remacs-no-such-upload-2673"))
        (setq remacs-test-eww-disabled-phase (list 'submit method table outside))
        (if outside
            ;; A disabled default submitter prevents implicit submission entirely.
            (progn
              (goto-char (car (remacs-test-eww-field (or submit-id "_submit"))))
              (cl-letf (((symbol-function 'eww-browse-url)
                         (lambda (&rest _) (error "Disabled default submitter navigated"))))
                (eww-submit)))
          (unless (let ((actual (remacs-test-eww-disabled-query method (or submit-id "_submit"))))
                    (if (functionp expected-query) (funcall expected-query actual)
                      (equal actual expected-query)))
            (error "Disabled control leaked into %S data: %S" method
                   (remacs-test-eww-disabled-query method (or submit-id "_submit")))))
        (dolist (id disabled-ids)
          (setq remacs-test-eww-disabled-phase (list 'disabled id method table outside))
          (let* ((entry (remacs-test-eww-field id))
                 (position (car entry))
                 (field (cdr entry))
                 (face (get-text-property position 'face))
                 (before (buffer-string))
                 (checked (plist-get field :checked))
                 (value (plist-get field :value))
                 (filename (plist-get field :filename)))
            (unless (and (plist-get field :disabled)
                         (get-text-property position 'read-only)
                         (not (get-text-property position 'inhibit-read-only))
                         (not (get-text-property position 'keymap))
                         (not (get-text-property position 'local-map))
                         (not (get-text-property position 'button))
                         (not (get-text-property position 'shr-tab-stop))
                         (not (get-text-property position 'field))
                         (or (eq face 'shadow) (and (listp face) (memq 'shadow face))))
              (error "Disabled control remains interactive: %S %S" id field))
            (unless (remacs-test-eww-edit-blocked (1+ position))
              (error "Disabled control accepted text: %S" id))
            (goto-char position)
            (cl-letf (((symbol-function 'read-file-name)
                       (lambda (&rest _) (error "Disabled control opened a file prompt")))
                      ((symbol-function 'popup-menu)
                       (lambda (&rest _) (error "Disabled control opened a popup")))
                      ((symbol-function 'mouse-set-point) #'ignore)
                      ((symbol-function 'eww-browse-url)
                       (lambda (&rest _) (error "Disabled control triggered navigation"))))
              (condition-case nil
                  (pcase (plist-get field :type)
                    ((or "checkbox" "radio") (eww-toggle-checkbox))
                    ("file" (eww-select-file))
                    ("select" (eww-change-select nil))
                    ("submit" (eww-submit)))
                (user-error nil)))
            (unless (and (equal (buffer-string) before)
                         (equal (plist-get field :checked) checked)
                         (equal (plist-get field :value) value)
                         (equal (plist-get field :filename) filename))
              (error "Disabled handler changed a control: %S" id))))
        (dolist (id readonly-ids)
          (setq remacs-test-eww-disabled-phase (list 'readonly id method table outside))
          (let* ((entry (remacs-test-eww-field id))
                 (position (car entry)))
            (unless (and (not (plist-get (cdr entry) :disabled))
                         (get-text-property position 'shr-tab-stop)
                         (remacs-test-eww-edit-blocked (1+ position)))
              (error "Readonly and disabled states were conflated: %S" id))))
        (unless outside
          (setq remacs-test-eww-disabled-phase (list 'normal-edit method table))
          (let* ((entry (condition-case nil (remacs-test-eww-field "normal") (error nil))))
            (when entry
              (goto-char (1+ (car entry)))
              (setq remacs-test-eww-disabled-phase
                    (list 'normal-insert method table
                          (get-text-property (point) 'read-only)
                          (get-text-property (point) 'inhibit-read-only)
                          (get-text-property (point) 'rear-nonsticky)))
              (let ((last-command-event ?X)) (self-insert-command 1))
              (setq remacs-test-eww-disabled-phase
                    (list 'normal-delete method table
                          (get-text-property (1- (point)) 'read-only)
                          (get-text-property (1- (point)) 'inhibit-read-only)
                          (get-text-property (point) 'inhibit-read-only)))
              (delete-char -1))))
        (when callback
          (setq remacs-test-eww-disabled-phase (list 'callback method table))
          (funcall callback)
          (unless (equal (remacs-test-eww-disabled-query method (or submit-id "_submit"))
                         (cadr expected))
            (error "Enabled control interaction changed submission incorrectly"))))))
  (dolist (method '("get" "post"))
    (dolist (table '(nil t))
      (remacs-test-eww-disabled
       method
       (list (dom-node 'input '((id . "locked") (name . "locked") (value . "secret") (disabled . "")))
             (dom-node 'input '((id . "check") (type . "checkbox") (name . "choice")
                                (value . "yes") (checked . "") (disabled . "")))
             (dom-node 'input '((id . "normal") (name . "normal") (value . "ok"))))
       "normal=ok" '("locked" "check") nil table)
      (remacs-test-eww-disabled
       method
       (list (dom-node 'input '((id . "ro") (name . "ro") (value . "keep") (readonly . "")))
             (dom-node 'textarea '((id . "area") (name . "area") (rows . "1")
                                   (disabled . "")) "secret")
             (dom-node 'textarea '((id . "roarea") (name . "roarea") (rows . "1")
                                   (readonly . "")) "keeparea")
             (dom-node 'input '((id . "hidden") (type . "hidden") (name . "hidden")
                                (value . "secret") (disabled . "")))
             (dom-node 'input '((id . "upload") (type . "file") (name . "upload") (disabled . "")))
             (dom-node 'select '((id . "single") (name . "single") (disabled . "false"))
                       (dom-node 'option '((value . "secret")) "Secret"))
             (dom-node 'button '((id . "button") (name . "button") (value . "secret")
                                 (disabled . "")) "Blocked")
             (dom-node 'input '((id . "input-button") (type . "submit") (name . "stop")
                                (value . "secret") (disabled . "")))
             (dom-node 'input '((id . "normal") (name . "normal") (value . "ok"))))
       "ro=keep&roarea=keeparea&normal=ok" '("area" "upload" "single" "button" "input-button")
       '("ro" "roarea") table)
      (remacs-test-eww-disabled
       method
       (list (dom-node 'fieldset '((disabled . ""))
                       (dom-node 'legend nil
                                 (dom-node 'input '((id . "legend") (name . "legend") (value . "open")))
                                 (dom-node 'input '((id . "legend-own") (name . "legend-own")
                                                    (value . "secret") (disabled . ""))))
                       (dom-node 'input '((id . "locked") (name . "locked") (value . "secret")))
                       (dom-node 'legend nil
                                 (dom-node 'input '((id . "second-legend") (name . "second")
                                                    (value . "secret"))))
                       (dom-node 'fieldset nil
                                 (dom-node 'legend nil
                                           (dom-node 'input '((id . "nested-legend") (name . "nested")
                                                              (value . "secret"))))))
             (dom-node 'input '((id . "normal") (name . "normal") (value . "ok"))))
       "legend=open&normal=ok" '("locked" "legend-own" "second-legend" "nested-legend") nil table)
      (remacs-test-eww-disabled
       method (list (dom-node 'input '((id . "locked") (name . "locked") (value . "secret"))))
       "" '("locked" "_submit") nil table 'table "locked")
      (remacs-test-eww-disabled
       method
       (list (dom-node 'input '((id . "group-text") (name . "group") (value . "secret") (disabled . "")))
             (dom-node 'input '((id . "group-check") (type . "checkbox") (name . "group")
                                (value . "check") (checked . "") (disabled . "")))
             (dom-node 'input '((id . "group-off") (type . "radio") (name . "group")
                                (value . "no") (checked . "") (disabled . "")))
             (dom-node 'input '((id . "group-on") (type . "radio") (name . "group") (value . "yes")))
             (dom-node 'input '((id . "normal") (name . "normal") (value . "ok"))))
       '("normal=ok" "group=yes&normal=ok") '("group-text" "group-check" "group-off") nil table nil nil
       (lambda ()
         (goto-char (car (remacs-test-eww-field "group-on")))
         (eww-toggle-checkbox)
         (unless (and (not (plist-get (cdr (remacs-test-eww-field "group-off")) :checked))
                      (plist-get (cdr (remacs-test-eww-field "group-check")) :checked)
                      (get-text-property (car (remacs-test-eww-field "group-off")) 'read-only)
                      (save-excursion
                        (goto-char (car (remacs-test-eww-field "group-text")))
                        (looking-at "secret")))
           (error "Radio grouping changed another disabled control type"))))
      (remacs-test-eww-disabled
       method
       (list (dom-node 'select '((id . "live-select") (name . "choice") (readonly . ""))
                       (dom-node 'option '((value . "x") (disabled . "") (selected . "")) "Bad")
                       (dom-node 'option '((value . "x")) "Good"))
             (dom-node 'input '((id . "normal") (name . "normal") (value . "ok"))))
       '("normal=ok" "choice=x&normal=ok") nil nil table nil nil
       (lambda ()
         (let* ((entry (remacs-test-eww-field "live-select"))
                (field (cdr entry))
                (choice 0))
           (cl-letf (((symbol-function 'mouse-set-point) #'ignore)
                     ((symbol-function 'popup-menu)
                      (lambda (menu &rest _)
                        (let ((item (nth (1+ choice) menu)))
                          (unless (eq (not (aref item 2)) (= choice 0))
                            (error "Single option enabled state was wrong"))
                          (call-interactively (aref item 1))))))
             (goto-char (car entry))
             (eww-change-select nil)
             (unless (equal (eww-select-display field) "Bad")
               (error "Disabled option changed via direct callback"))
             (setq choice 1)
             (goto-char (car entry))
             (eww-change-select nil)
             (unless (equal (eww-select-display field) "Good")
               (error "Equal-valued enabled option did not become selected"))))))))
  (let ((file (make-temp-file "remacs-eww-enabled-upload-")))
    (unwind-protect
        (progn
          (with-temp-file file (insert "enabled-file-payload-2673"))
          (let ((remacs-test-eww-disabled-setup
                 (lambda ()
                   (goto-char (car (remacs-test-eww-field "enabled-upload")))
                   (cl-letf (((symbol-function 'read-file-name) (lambda (&rest _) file)))
                     (eww-select-file)))))
            (remacs-test-eww-disabled
             "post"
             (list (dom-node 'input '((id . "enabled-upload") (type . "file") (name . "upload")
                                      (readonly . "")))
                   (dom-node 'input '((id . "upload") (type . "file") (name . "blocked")
                                      (disabled . "")))
                   (dom-node 'textarea '((id . "description") (name . "description") (rows . "1")
                                         (readonly . "")) "readonly-description-2673")
                   (dom-node 'input '((type . "hidden") (name . "blocked-hidden")
                                      (value . "disabled-secret-2673") (disabled . ""))))
             (lambda (body)
               (and (string-search "enabled-file-payload-2673" body)
                    (string-search "readonly-description-2673" body)
                    (not (string-search "disabled-secret-2673" body))
                    (not (string-search "name=\"blocked\"" body))))
             '("upload") '("description") nil nil nil nil "multipart/form-data")))
      (delete-file file)))
  (with-temp-buffer
    (let* ((form (list (cons :method "post") (cons :action "https://example.test/search")))
           (blocked (list :eww-form form :name "blocked" :type "checkbox"
                          :value "secret" :checked t :disabled t))
           (normal (list :eww-form form :name "normal" :type "text" :value "ok"))
           (eww-data (list :url "https://example.test/page"))
           body)
      (insert "[X] ok")
      (put-text-property 1 4 'eww-form blocked)
      (put-text-property 5 7 'eww-form normal)
      (goto-char 1)
      (eww-toggle-checkbox)
      (unless (plist-get blocked :checked) (error "Stored disabled flag was ignored"))
      (goto-char 5)
      (cl-letf (((symbol-function 'eww-browse-url)
                 (lambda (&rest _) (setq body url-request-data))))
        (eww-submit))
      (unless (equal body "normal=ok") (error "Stored disabled value was sent"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let phase_symbol = interp.intern("remacs-test-eww-disabled-phase");
            let phase = interp.symbol_value(phase_symbol);
            let frames = interp
                .last_error_stack
                .borrow()
                .iter()
                .rev()
                .take(24)
                .map(|frame| match &frame.function {
                    remacs_runtime::lisp::Value::Sym(id) => interp.symbol_name(*id),
                    _ => "<anonymous>".to_owned(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{} {} phase={}\n{}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase),
                frames
            )
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn disabled_controls_cannot_edit_or_submit_but_readonly_values_submit() {
    let mut interp = Interp::new();
    check(&mut interp, PRIMITIVE_CASES);
    eprintln!("default EWW disabled control rendering and submission");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW disabled control rendering and submission");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW disabled control rendering and submission");
    check(&mut interp, CASES);
}
