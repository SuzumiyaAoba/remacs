use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defun remacs-test-eww-multi-check (method field state)
    (let (observed submit)
      (goto-char (point-min))
      (while (< (point) (point-max))
        (when (equal (plist-get (get-text-property (point) 'eww-form) :type) "submit")
          (setq submit (point)))
        (forward-char))
      (goto-char submit)
      (cl-letf (((symbol-function 'eww-browse-url)
                 (lambda (url &rest _) (setq observed (list url url-request-data)))))
        (eww-submit))
      (let ((query (if (equal method "post") (nth 1 observed)
                     (cdr (url-path-and-query (url-generic-parse-url (car observed)))))))
        (unless (and (equal (plist-get field :value) (nth 0 state))
                     (equal (eww-select-display field) (nth 1 state))
                     (equal query (nth 2 state)))
          (error "Multiple selection state/order failed: %S / %S expected %S"
                 (plist-get field :value) observed state)))
      (goto-char (point-min))
      (while (and (< (point) (point-max))
                  (not (eq (get-text-property (point) 'eww-form) field)))
        (forward-char))
      (let* ((start (point))
             (end (next-single-property-change start 'eww-form nil (point-max)))
             (text (buffer-substring-no-properties start end))
             (width (string-width text))
             (display (nth 1 state)))
        (unless (and buffer-read-only (> end start)
                     (>= width (max 1 (string-width display)))
                     (equal text (concat display (make-string (- width (string-width display)) ? ))))
          (error "Multiple selection lost its padded field: %S" text))
        (dotimes (offset (- end start))
          (unless (and (eq (get-text-property (+ start offset) 'eww-form) field)
                       (eq (get-text-property (+ start offset) 'keymap) eww-select-map)
                       (get-text-property (+ start offset) 'button))
            (error "Multiple selection lost properties at %S" offset))))))
  (defun remacs-test-eww-multi (method options state changes table &optional disabled history)
    (with-temp-buffer
      (let* ((eww-data (list :url "https://example.test/page" :title "Test"))
             (eww-history nil)
             (eww-history-position 0)
             (shr-use-fonts nil)
             (shr-width 100)
             (shr-external-rendering-functions
              '((form . eww-tag-form) (select . eww-tag-select)
                (input . eww-tag-input) (button . eww-form-submit)))
             (select (append (list 'select (append '((name . "tag") (multiple . ""))
                                                   (when disabled '((disabled . "")))))
                             options))
             (children (list (dom-node 'input '((name . "before") (value . "head")))
                             select
                             (dom-node 'input '((type . "hidden") (name . "after") (value . "tail")))
                             (dom-node 'button nil "Send")))
             (form (append (list 'form (list (cons 'method method)
                                            (cons 'action "https://example.test/search")))
                           (if table
                               (list (append (list 'table nil)
                                             (list (append (list 'tr nil)
                                                           (mapcar (lambda (node) (dom-node 'td nil node))
                                                                   children)))))
                             children)))
             field)
        (shr-insert-document (list 'html nil (list 'body nil form)))
        (goto-char (point-min))
        (while (and (< (point) (point-max)) (not field))
          (let ((value (get-text-property (point) 'eww-form)))
            (when (equal (plist-get value :name) "tag") (setq field value)))
          (unless field (forward-char)))
        (unless field (error "Multiple select has no field"))
        (setq buffer-read-only t)
        (remacs-test-eww-multi-check method field state)
        (dolist (change changes)
          (let ((choice (nth 0 change))
                (next-state (nth 1 change))
                (blocked (nth 2 change))
                popped)
            (cl-letf (((symbol-function 'mouse-set-point) #'ignore)
                      ((symbol-function 'popup-menu)
                       (lambda (menu &rest _)
                         (setq popped t)
                         (when disabled (error "Disabled multiple select opened its popup"))
                         (let ((index 0))
                           (dolist (item (cdr menu))
                             (let ((marker (if (memq index (nth 3 state))
                                               eww-form-checkbox-selected-symbol
                                             eww-form-checkbox-symbol)))
                               (unless (string-prefix-p (concat marker " ") (aref item 0))
                                 (error "Popup did not expose selection state: %S" item)))
                             (setq index (1+ index))))
                         (let ((item (nth (1+ choice) menu)))
                           (unless (eq (not (aref item 2)) (not (null blocked)))
                             (error "Popup option has incorrect enabled state"))
                           ;; Even a direct call must not toggle a disabled option.
                           (call-interactively (aref item 1))))))
              (eww-change-select nil))
            (unless (or disabled popped) (error "Multiple select did not open a popup"))
            (setq state next-state)
            (remacs-test-eww-multi-check method field state)))
        (when history
          (eww-save-history)
          (let ((saved (car eww-history))
                (inhibit-read-only t))
            (erase-buffer)
            (insert "Other page")
            (eww-restore-history saved))
          (remacs-test-eww-multi-check method field state)))))
  (dolist (method '("get" "post"))
    (dolist (table '(nil t))
      (remacs-test-eww-multi
       method
       (list (dom-node 'option '((value . "red") (selected . "")) "Red")
             (dom-node 'option '((value . "blue") (selected . "")) "Blue")
             (dom-node 'option '((value . "green")) "Green"))
       '(("red" "blue") "Red, Blue" "before=head&tag=red&tag=blue&after=tail" (0 1))
       '((2 (("red" "blue" "green") "Red, Blue, Green"
             "before=head&tag=red&tag=blue&tag=green&after=tail" (0 1 2)))
         (0 (("blue" "green") "Blue, Green" "before=head&tag=blue&tag=green&after=tail" (1 2)))
         (1 (("green") "Green" "before=head&tag=green&after=tail" (2)))
         (2 (nil "" "before=head&after=tail" nil)))
       table nil t)
      (remacs-test-eww-multi
       method
       (list (dom-node 'option '((value . "x") (selected . "")) "First")
             (dom-node 'option '((value . "x")) "Second"))
       '(("x") "First" "before=head&tag=x&after=tail" (0))
       '((1 (("x" "x") "First, Second" "before=head&tag=x&tag=x&after=tail" (0 1)))
         (0 (("x") "Second" "before=head&tag=x&after=tail" (1))))
       table nil t)
      (remacs-test-eww-multi
       method (list (dom-node 'option nil "Red") (dom-node 'option nil "Blue"))
       '(nil "" "before=head&after=tail" nil)
       '((1 (("Blue") "Blue" "before=head&tag=Blue&after=tail" (1))))
       table)
      (remacs-test-eww-multi
       method (list (dom-node 'option '((value . "") (selected . "")) "Empty")
                    (dom-node 'option nil ""))
       '(("") "Empty" "before=head&tag=&after=tail" (0))
       '((1 (("" "") "Empty, " "before=head&tag=&tag=&after=tail" (0 1)))
         (0 (("") "" "before=head&tag=&after=tail" (1))))
       table)
      (remacs-test-eww-multi
       method (list (dom-node 'option '((value . "a") (selected . "")) "A")
                    (dom-node 'optgroup '((disabled . ""))
                              (dom-node 'option '((value . "b") (selected . "")) "B"))
                    (dom-node 'option '((value . "c") (selected . "") (disabled . "")) "C"))
       '(("a" "b" "c") "A, B, C" "before=head&tag=a&after=tail" (0 1 2))
       '((1 (("a" "b" "c") "A, B, C" "before=head&tag=a&after=tail" (0 1 2)) t)
         (2 (("a" "b" "c") "A, B, C" "before=head&tag=a&after=tail" (0 1 2)) t)
         (0 (("b" "c") "B, C" "before=head&after=tail" (1 2))))
       table)
      (remacs-test-eww-multi
       method (list (dom-node 'option '((value . "a") (selected . "")) "A"))
       '(("a") "A" "before=head&after=tail" (0))
       '((0 (("a") "A" "before=head&after=tail" (0)) t)) table t)))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn multiple_selects_toggle_and_submit_in_document_order() {
    let mut interp = Interp::new();
    eprintln!("default EWW multiple select rendering and submission");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW multiple select rendering and submission");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW multiple select rendering and submission");
    check(&mut interp, CASES);
}
