use remacs_runtime::lisp::{Flow, Interp};

const FIELD_CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (with-temp-buffer
    (insert "AA field ZZ\nsecond\nthird\n")
    (put-text-property 4 9 'field 'input)
    ;; Recorded from GNU 31.1, including start/end edges and cross-line scans.
    (dolist (case '((1 nil 1 12) (3 nil 1 12) (4 nil 1 4) (5 nil 4 9)
                    (8 nil 4 9) (9 nil 4 9) (10 nil 1 12) (12 nil 1 12)
                    (4 0 1 1) (5 0 4 4) (9 0 4 4)
                    (5 -1 4 4) (5 1 4 9) (5 2 13 19) (5 3 20 25)
                    (9 2 13 19) (13 0 1 12) (13 1 13 19) (13 3 26 26)))
      (goto-char (nth 0 case))
      (let ((before (point)))
        (unless (and (= (line-beginning-position (nth 1 case)) (nth 2 case))
                     (= (line-end-position (nth 1 case)) (nth 3 case))
                     (= (point) before))
          (error "Field-constrained line positions changed: %S" case))))
    (goto-char 5)
    (unless (and (= (line-beginning-position) 4) (= (line-end-position) 9)
                 (= (pos-bol) 1) (= (pos-eol) 12))
      (error "Pure line and field queries were conflated"))
    (let ((inhibit-field-text-motion t))
      (unless (and (= (line-beginning-position) 1) (= (line-end-position) 12)
                   (= (point) 5))
        (error "Field motion inhibition did not bypass the constraint")))
    (dolist (arg (list (point-marker) 1.0 "1" t))
      (dolist (function '(line-beginning-position line-end-position))
        (unless (equal (condition-case err (funcall function arg)
                         (error (list (car err) (cadr err))))
                       '(wrong-type-argument integerp))
          (error "Line query accepted non-integer count %S" arg)))))
  (with-temp-buffer
    (insert "AAAA BBBB CCCC\n")
    (setq-local text-property-default-nonsticky '((field . t)))
    (let ((q (list :id "q")) (ro (list :id "ro")))
      (put-text-property 1 5 'field q)
      (put-text-property 6 10 'field ro)
      (put-text-property 1 15 'front-sticky t)
      (put-text-property 6 15 'rear-nonsticky t)
      (dolist (case '((1 1 5 "q") (2 1 5 "q") (4 1 5 "q") (5 5 6 nil)
                      (6 6 10 "ro") (7 6 10 "ro") (9 6 10 "ro")
                      (10 10 15 nil) (11 10 15 nil) (14 10 15 nil) (15 10 15 nil)))
        (goto-char (car case))
        (unless (equal (list (point) (line-beginning-position) (line-end-position)
                             (plist-get (get-pos-property (point) 'field) :id)) case)
          (error "Sticky adjacent field query changed: %S got %S" case
                 (list (point) (line-beginning-position) (line-end-position)
                       (plist-get (get-pos-property (point) 'field) :id)))))))
  (with-temp-buffer
    (insert "abcdef")
    (put-text-property 1 4 'field 'outside)
    (narrow-to-region 3 7)
    (goto-char (point-min))
    (unless (null (get-pos-property (point) 'field))
      (error "Position property inherited from before the accessible beginning")))
  t)
"##;

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defvar remacs-test-eww-button-phase nil)
  (defun remacs-test-eww-button-field (id)
    (save-excursion
      (goto-char (point-min))
      (let (found)
        (while (and (< (point) (point-max)) (not found))
          (let ((field (get-text-property (point) 'eww-form)))
            (when (equal (dom-attr (plist-get field :dom) 'id) id)
              (setq found (cons (point) field))))
          (unless found (forward-char)))
        (or found
            (let ((scan (point-min)) visible)
              (while (< scan (point-max))
                (let* ((field (get-text-property scan 'eww-form))
                       (end (next-single-property-change scan 'eww-form nil (point-max))))
                  (when (keywordp (car field))
                    (push (list scan end (dom-attr (plist-get field :dom) 'id)) visible))
                  (setq scan end)))
              (error "Missing button control %S; visible=%S" id (nreverse visible)))))))
  (defun remacs-test-eww-button-text (id)
    (let* ((entry (remacs-test-eww-button-field id)) (start (car entry)))
      (buffer-substring-no-properties
       start (next-single-property-change start 'eww-form nil (point-max)))))
  (defun remacs-test-eww-button-activate (id &optional key)
    (setq remacs-test-eww-button-phase (list 'activate id key))
    (goto-char (car (remacs-test-eww-button-field id)))
    (let ((command (key-binding (kbd (or key "RET"))))
          (last-nonmenu-event nil))
      (unless (commandp command) (error "Button lacks a real command binding: %S" id))
      (call-interactively command)))
  (defun remacs-test-eww-button-flush ()
    (when eww--change-tracker-id (eww--track-changes eww--change-tracker-id)))
  (defun remacs-test-eww-button-edit (id &optional force)
    (setq remacs-test-eww-button-phase (list 'editing id force))
    (goto-char (1+ (car (remacs-test-eww-button-field id))))
    (let ((last-command-event ?X) (inhibit-read-only force)) (self-insert-command 1))
    (setq remacs-test-eww-button-phase (list 'processing id force))
    (remacs-test-eww-button-flush)
    (when force (remacs-test-eww-button-field "readonly"))
    (setq remacs-test-eww-button-phase (list 'after-edit id force)))
  (defun remacs-test-eww-button-snapshot (ids)
    (mapcar
     (lambda (id)
       (let ((field (cdr (remacs-test-eww-button-field id))))
         (list id (plist-get field :type) (copy-tree (plist-get field :value))
               (not (null (plist-get field :checked)))
               (plist-get field :filename)
               (mapcar (lambda (item) (list (plist-get item :value)
                                           (not (null (plist-get item :selected)))))
                       (eww--form-items field))
               (remacs-test-eww-button-text id)))) ids))
  (defun remacs-test-eww-button-page (body)
    (eww-mode)
    (setq-local eww-data (list :url "https://example.test/page" :title "Buttons"))
    (let ((shr-use-fonts nil) (shr-width 120))
      (eww-display-document (eww-document-base "https://example.test/page"
                                             (dom-node 'html nil body)))))
  ;; Baseline: a plain button must neither submit nor apply submitter overrides.
  (with-temp-buffer
    (remacs-test-eww-button-page
     (dom-node 'body nil
               (dom-node 'form '((action . "/submit") (method . "get"))
                         (dom-node 'input '((name . "q") (value . "hello")))
                         (dom-node 'button '((id . "preview") (type . "button")
                                             (name . "preview") (value . "1")
                                             (formaction . "/wrong") (formmethod . "post")) "Preview"))))
    (cl-letf (((symbol-function 'eww-browse-url)
               (lambda (&rest _) (error "Plain button submitted the form"))))
      (remacs-test-eww-button-activate "preview")
      (unless (condition-case nil (progn (eww-submit) nil) (user-error t))
        (error "Direct submit accepted a plain button")))
    (unless (and (equal (plist-get (cdr (remacs-test-eww-button-field "preview")) :type) "button")
                 (string-prefix-p "Preview" (remacs-test-eww-button-text "preview")))
      (error "Button type/child label was replaced by its submission value")))
  (dolist (tag '(button input))
    (dolist (table '(nil t))
      (with-temp-buffer
        (setq remacs-test-eww-button-phase (list 'reset tag table))
        (let* ((button (lambda (id type label disabled)
                         (let ((attrs (list (cons 'id id) (cons 'type type)
                                            (cons 'name id) (cons 'value (concat id "-token")))))
                           (when disabled (push '(disabled . "") attrs))
                           (if (eq tag 'input) (dom-node 'input attrs)
                             (dom-node 'button attrs label)))))
               (children
                (list
                 "Form A before "
                 (dom-node 'input '((id . "text") (name . "text") (value . "a  ") (size . "9")))
                 (dom-node 'input '((id . "password") (name . "password") (type . "password")
                                    (value . "initial-secret") (size . "16")))
                 (dom-node 'textarea '((id . "area") (name . "area") (rows . "3") (cols . "8")) "a\nb  ")
                 (dom-node 'input '((id . "readonly") (name . "readonly") (value . "RO") (readonly . "")))
                 (dom-node 'input '((id . "disabled") (name . "disabled") (value . "D") (disabled . "")))
                 (dom-node 'input '((id . "check") (name . "check") (type . "checkbox") (checked . "")))
                 (dom-node 'input '((id . "unchecked") (name . "unchecked") (type . "checkbox")))
                 (dom-node 'input '((id . "radio-a") (name . "radio") (type . "radio") (value . "a") (checked . "")))
                 (dom-node 'input '((id . "radio-b") (name . "radio") (type . "radio") (value . "b")))
                 (dom-node 'input '((id . "disabled-check") (type . "checkbox") (checked . "") (disabled . "")))
                 (dom-node 'select '((id . "single") (name . "single"))
                           (dom-node 'option '((value . "same")) "First")
                           (dom-node 'option '((value . "same") (selected . "")) "Second"))
                 (dom-node 'select '((id . "multi") (name . "multi") (multiple . ""))
                           (dom-node 'option '((value . "x") (selected . "")) "X")
                           (dom-node 'option '((value . "y")) "Y")
                           (dom-node 'option '((value . "z") (selected . "") (disabled . "")) "Z"))
                 (dom-node 'input '((id . "upload") (name . "upload") (type . "file")))
                 (dom-node 'input '((id . "hidden") (name . "hidden") (type . "hidden") (value . "initial-hidden")))
                 (funcall button "preview" "BuTtOn" "Preview" nil)
                 (funcall button "reset" "ReSeT" "Reset" nil)
                 (funcall button "blocked-reset" "reset" "Blocked reset" t)
                 (dom-node 'button '((id . "send") (name . "send") (value . "yes")) "Send")
                 " Form A after"))
               (form (append (list 'form '((action . "/submit") (method . "post")))
                             (if table
                                 (list (dom-node 'table nil
                                                 (append (list 'tr nil)
                                                         (mapcar (lambda (child) (dom-node 'td nil child))
                                                                 children))))
                               children)))
               (ids '("text" "password" "area" "readonly" "disabled" "check" "unchecked"
                      "radio-a" "radio-b" "disabled-check" "single" "multi" "upload"))
               initial owner other-value)
          (remacs-test-eww-button-page
           (dom-node 'body nil form
                     (dom-node 'form '((action . "/other"))
                               (dom-node 'input '((id . "other") (name . "other") (value . "other"))))))
          (setq initial (remacs-test-eww-button-snapshot ids)
                owner (plist-get (cdr (remacs-test-eww-button-field "text")) :eww-form))
          (dolist (id '("text" "password" "area" "other")) (remacs-test-eww-button-edit id))
          (dolist (id '("readonly" "disabled")) (remacs-test-eww-button-edit id t))
          (setq other-value (remacs-test-eww-button-text "other"))
          (dolist (id '("check" "unchecked" "radio-b"))
            (goto-char (car (remacs-test-eww-button-field id))) (eww-toggle-checkbox))
          (plist-put (cdr (remacs-test-eww-button-field "disabled-check")) :checked nil)
          (dolist (id '("single" "multi"))
            (let* ((entry (remacs-test-eww-button-field id)) (field (cdr entry))
                   (items (eww--form-items field)))
              (dolist (item items) (plist-put item :selected nil))
              (plist-put (nth 0 items) :selected t)
              (plist-put field :value (if (equal id "multi") '("x") "same"))
              (goto-char (car entry)) (eww-update-field (eww-select-display field))))
          (goto-char (car (remacs-test-eww-button-field "upload")))
          (cl-letf (((symbol-function 'read-file-name)
                     (lambda (&rest _) "/private/tmp/remacs-no-such-file-2676")))
            (eww-select-file))
          (dolist (entry owner)
            (when (and (consp entry) (eq (car entry) 'hidden))
              (plist-put (cdr entry) :value "changed-hidden")))
          (let ((before (buffer-string)))
            (cl-letf (((symbol-function 'eww-browse-url)
                       (lambda (&rest _) (error "Non-submit button navigated"))))
              (remacs-test-eww-button-activate "preview" "<mouse-2>")
              (goto-char (car (remacs-test-eww-button-field "blocked-reset")))
              (condition-case nil (eww-activate-form-button) (user-error nil)))
            (unless (equal before (buffer-string)) (error "Plain/disabled button changed the form")))
          (cl-letf (((symbol-function 'eww-browse-url)
                     (lambda (&rest _) (error "Reset submitted the form"))))
            (remacs-test-eww-button-activate "reset"))
          (remacs-test-eww-button-flush)
          (unless (and (equal (remacs-test-eww-button-snapshot ids) initial)
                       (equal (remacs-test-eww-button-text "other") other-value)
                       (cl-every (lambda (id)
                                   (eq (plist-get (cdr (remacs-test-eww-button-field id)) :eww-form) owner)) ids)
                       (cl-every (lambda (entry)
                                   (or (not (and (consp entry) (eq (car entry) 'hidden)))
                                       (equal (plist-get (cdr entry) :value) "initial-hidden"))) owner)
                       (not (track-changes-inconsistent-state-p)))
            (error "Reset mismatch: fields=%S other=%S owners=%S hidden=%S tracker=%S"
                   (cl-remove-if-not #'identity
                    (cl-mapcar (lambda (old new) (unless (equal old new) (list old new)))
                               initial (remacs-test-eww-button-snapshot ids)))
                   (equal (remacs-test-eww-button-text "other") other-value)
                   (mapcar (lambda (id) (eq (plist-get (cdr (remacs-test-eww-button-field id)) :eww-form) owner)) ids)
                   (cl-loop for entry in owner when (and (consp entry) (eq (car entry) 'hidden))
                            collect (plist-get (cdr entry) :value))
                   (track-changes-inconsistent-state-p)))
          (let* ((entry (remacs-test-eww-button-field "password")) (start (car entry)))
            (unless (equal (get-text-property start 'display) (make-string (length "initial-secret") ?*))
              (error "Reset exposed the initial password")))
          (dolist (id '("readonly" "disabled"))
            (goto-char (1+ (car (remacs-test-eww-button-field id))))
            (unless (condition-case nil
                        (progn (let ((last-command-event ?X)) (self-insert-command 1)) nil)
                      ((buffer-read-only text-read-only) t))
              (error "Reset removed read-only/disabled protection")))
          (remacs-test-eww-button-edit "text")
          (unless (equal (plist-get (cdr (remacs-test-eww-button-field "text")) :value) "aX  ")
            (error "Reset left subsequent input untracked"))
          (cl-letf (((symbol-function 'eww-browse-url)
                     (lambda (&rest _) (error "Second reset navigated"))))
            (remacs-test-eww-button-activate "reset" "C-c C-c"))
          (remacs-test-eww-button-flush)
          (unless (equal (remacs-test-eww-button-snapshot ids) initial)
            (error "Reset failed after subsequent editing"))
          ;; Clear an entire field, navigate away, then reset the restored page.
          ;; Numeric history anchors must recover even a zero-length field.
          (setq remacs-test-eww-button-phase (list 'history tag table))
          (let* ((entry (remacs-test-eww-button-field "text")) (start (car entry))
                 (end (next-single-property-change start 'eww-form nil (point-max)))
                 (size (buffer-size)))
            (delete-region start end)
            (remacs-test-eww-button-flush)
            (when-let* ((remaining (condition-case nil (remacs-test-eww-button-field "text") (error nil))))
              (error "Full-field delete left a control: range=%S size=%S value=%S text=%S"
                     (list start end) (list size (buffer-size))
                     (plist-get (cdr remaining) :value)
                     (remacs-test-eww-button-text "text"))))
          (eww-save-history)
          (let ((saved (car eww-history)))
            (remacs-test-eww-button-page (dom-node 'body nil "Other page"))
            (eww-restore-history saved))
          (cl-letf (((symbol-function 'eww-browse-url)
                     (lambda (&rest _) (error "History reset navigated"))))
            (remacs-test-eww-button-activate "reset"))
          (remacs-test-eww-button-flush)
          (unless (and (equal (remacs-test-eww-button-snapshot ids) initial)
                       (equal (remacs-test-eww-button-text "other") other-value))
            (error "History reset lost a cleared field or changed another form"))
          (let (request)
            (cl-letf (((symbol-function 'eww-browse-url)
                       (lambda (url &rest _) (setq request (list url url-request-data)))))
              (remacs-test-eww-button-activate "send"))
            (unless (equal request
                           '("https://example.test/submit"
                             "text=a++&password=initial-secret&area=a%0Ab++&readonly=RO&check=on&radio=a&single=same&multi=x&upload=&hidden=initial-hidden&send=yes"))
              (error "Reset/button state leaked into submission: %S" request)))))))
  ;; Forms in adjacent outer-table cells must retain their independent fragments.
  (with-temp-buffer
    (setq remacs-test-eww-button-phase 'adjacent-table-forms)
    (remacs-test-eww-button-page
     (dom-node 'body nil
               (dom-node 'table nil
                         (dom-node 'tr nil
                                   (dom-node 'td nil
                                             (dom-node 'form nil
                                                       (dom-node 'input '((id . "a-text") (value . "A") (size . "5")))
                                                       (dom-node 'textarea '((id . "a-area") (rows . "2") (cols . "5")) "a\nb")
                                                       (dom-node 'button '((id . "a-reset") (type . "reset")) "Reset")))
                                   (dom-node 'td nil
                                             (dom-node 'form nil
                                                       (dom-node 'input '((id . "b-text") (value . "B") (size . "5")))
                                                       (dom-node 'textarea '((id . "b-area") (rows . "2") (cols . "5")) "c\nd")
                                                       (dom-node 'button '((id . "b-reset") (type . "reset")) "Reset")))))))
    (let ((initial (remacs-test-eww-button-snapshot '("a-text" "a-area"))) other)
      (remacs-test-eww-button-edit "a-text")
      (remacs-test-eww-button-edit "b-text")
      (setq other (remacs-test-eww-button-snapshot '("b-text" "b-area")))
      (cl-letf (((symbol-function 'eww-browse-url)
                 (lambda (&rest _) (error "Adjacent form reset navigated"))))
        (remacs-test-eww-button-activate "a-reset"))
      (remacs-test-eww-button-flush)
      (unless (and (equal (remacs-test-eww-button-snapshot '("a-text" "a-area")) initial)
                   (equal (remacs-test-eww-button-snapshot '("b-text" "b-area")) other))
        (error "Reset crossed adjacent table forms"))))
  (with-temp-buffer
    (remacs-test-eww-button-page
     (dom-node 'body nil (dom-node 'button '((id . "orphan") (type . "reset")) "Reset")))
    (cl-letf (((symbol-function 'eww-browse-url)
               (lambda (&rest _) (error "Ownerless reset navigated"))))
      (remacs-test-eww-button-activate "orphan")))
  ;; Missing/invalid button types remain submits; input reset/button also never submit.
  (dolist (type '(nil "" "submit" "SuBmIt" "invalid"))
    (with-temp-buffer
      (let (request)
        (remacs-test-eww-button-page
         (dom-node 'body nil
                   (dom-node 'form '((action . "/send") (method . "get"))
                             (dom-node 'button (list '(id . "default") '(name . "go") '(value . "yes")
                                                     (cons 'type type)) "Default")
                             (dom-node 'input '((id . "q") (name . "q") (value . "hello")))
                             (dom-node 'input '((type . "button") (name . "preview") (value . "bad")))
                             (dom-node 'input '((type . "reset") (name . "reset") (value . "bad"))))))
        (cl-letf (((symbol-function 'eww-browse-url)
                   (lambda (url &rest _) (setq request url))))
          (goto-char (car (remacs-test-eww-button-field "q"))) (eww-submit))
        (unless (equal request "https://example.test/send?go=yes&q=hello")
          (error "Default submitter classification failed: %S %S" type request)))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let id = interp.intern("remacs-test-eww-button-phase");
            let phase = interp.symbol_value(id);
            let frames = interp
                .last_error_stack
                .borrow()
                .iter()
                .rev()
                .take(20)
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
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn button_types_control_activation_and_reset_only_the_owning_form() {
    let mut interp = Interp::new();
    check(&mut interp, FIELD_CASES);
    eprintln!("default EWW button types and reset");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW button types and reset");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW button types and reset");
    check(&mut interp, CASES);
}
