use remacs_runtime::lisp::{Flow, Interp, Value};

const CASES: &str = r#"
(progn
  (require 'eww)
  (require 'cl-lib)
  (defun remacs-test-eww-field (old new expected &rest options)
    (with-temp-buffer
      (insert "[" old "]TAIL")
      (let ((form (list :name "field"))
            (map (make-sparse-keymap))
            (offset (or (plist-get options :offset) 0)))
        (add-text-properties 2 (+ 2 (length old))
                             (list 'eww-form form 'keymap map 'button t
                                   'face 'eww-form-select))
        (when (plist-get options :narrow)
          (narrow-to-region 2 (+ 2 (length old))))
        (goto-char (+ 2 (or (plist-get options :cursor) 0)))
        (setq buffer-read-only t)
        (let ((start (eww-update-field new offset)))
          (unless (and (= start (+ 2 offset)) buffer-read-only)
            (error "Field replacement returned %S or changed read-only state" start)))
        (when (plist-get options :narrow)
          (unless (and (= (point-min) 2)
                       (= (point-max) (+ 2 (length expected))))
            (error "Field replacement lost narrowing")))
        (widen)
        (unless (equal (buffer-string) (concat "[" expected "]TAIL"))
          (error "Field %S -> %S: expected %S, got %S"
                 old new expected (buffer-string)))
        (when (= offset 0)
          (dotimes (index (length expected))
            (unless (and (eq (get-text-property (+ 2 index) 'eww-form) form)
                         (eq (get-text-property (+ 2 index) 'keymap) map)
                         (get-text-property (+ 2 index) 'button))
              (error "Field padding lost properties at %S" index)))
          (goto-char 2)
          (unless (= (1+ (eww-end-of-field)) (+ 2 (length expected)))
            (error "Updated field has the wrong property boundary"))))))
  (defun remacs-test-eww-select (dom width selections)
    (with-temp-buffer
      (let ((eww-form nil)) (eww-tag-select dom))
      (insert "TAIL")
      (goto-char (point-min))
      (while (and (< (point) (point-max))
                  (not (get-text-property (point) 'eww-form)))
        (forward-char))
      (let* ((start (point))
             (form (get-text-property start 'eww-form))
             (end (1+ (eww-end-of-field)))
             (suffix (buffer-substring-no-properties end (point-max))))
        (unless (= (string-width (buffer-substring start end)) width)
          (error "Initial select width is wrong: %S" (buffer-string)))
        (setq buffer-read-only t)
        (dolist (selection selections)
          (goto-char start)
          (cl-letf (((symbol-function 'mouse-set-point) #'ignore)
                    ((symbol-function 'popup-menu)
                     (lambda (menu &rest _)
                       (call-interactively
                        (aref (nth (1+ (car selection)) menu) 1)))))
            (eww-change-select nil))
          (let* ((end (1+ (eww-end-of-field)))
                 (display (nth 2 selection))
                 (expected (concat display
                                   (make-string (- width (string-width display)) ? ))))
            (unless (and (equal (plist-get form :value) (nth 1 selection))
                         (equal (buffer-substring-no-properties start end) expected)
                         (equal (buffer-substring-no-properties end (point-max)) suffix)
                         buffer-read-only)
              (error "Select %S: expected %S and suffix %S, got %S"
                     selection expected suffix (buffer-string)))
            (dotimes (index (length expected))
              (unless (and (eq (get-text-property (+ start index) 'eww-form) form)
                           (eq (get-text-property (+ start index) 'keymap) eww-select-map)
                           (get-text-property (+ start index) 'button))
                (error "Select padding lost its field at %S" index))))))))
  (let ((fields
         '(("1234567" "x" "x      ")
           ("1234567" "x" "x      " :cursor 5)
           ("1234567" "7654321" "7654321")
           ("1234567" "longer value" "longer value")
           ("1234567" "" "       ")
           ("1234567" "日本" "日本   ")
           ("1234567" "é" "é      ")
           ("日本語" "x" "x     ")
           ("[]" "[x]" "[x]")
           ("1234567" "x" "x      " :narrow t :cursor 2)
           ("Browse1234" "x" "Browsex   " :offset 6)
           ("Browse1234" "x" "Browsex   " :offset 6 :cursor 2)
           ("Browse1234" "longer path" "Browselonger path" :offset 6))))
    (dolist (field fields)
      (setq remacs-test-eww-phase field)
      (apply #'remacs-test-eww-field field))
    (setq remacs-test-eww-phase 'ascii-menu)
    (remacs-test-eww-select
     '(select ((name . "choice"))
              (option ((value . "long")) "Longest choice")
              (option ((value . "short")) "X")
              (option ((value . "empty")) ""))
     14 '((1 "short" "X") (2 "empty" "") (0 "long" "Longest choice") (1 "short" "X")))
    (setq remacs-test-eww-phase 'wide-menu)
    (remacs-test-eww-select
     '(select ((name . "choice"))
              (option ((value . "wide")) "日本語")
              (option ((value . "ascii") (selected . "selected")) "abcde"))
     6 '((0 "wide" "日本語") (1 "ascii" "abcde") (0 "wide" "日本語")))
    (setq remacs-test-eww-phase 'combining-menu)
    (remacs-test-eww-select
     '(select ((name . "choice"))
              (option ((value . "combining")) "é")
              (option ((value . "ascii")) "ab"))
     2 '((1 "ascii" "ab") (0 "combining" "é")))
    (length fields)))
"#;

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => {
            let phase_symbol = interp.intern("remacs-test-eww-phase");
            let phase = interp.symbol_value(phase_symbol);
            let frames = interp
                .last_error_stack
                .borrow()
                .iter()
                .rev()
                .take(30)
                .map(|frame| match &frame.function {
                    Value::Sym(symbol) => {
                        let name = interp.symbol_name(*symbol);
                        if matches!(
                            name.as_str(),
                            "require" | "load" | "load-with-code-conversion"
                        ) {
                            format!("{name} {}", interp.prin1_to_string(&frame.arguments))
                        } else {
                            name
                        }
                    }
                    _ => "<anonymous>".to_owned(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{} {} (phase {}, source {:?})\n{frames}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase),
                source.chars().take(80).collect::<String>(),
            )
        }
        other => panic!("{other:?}"),
    })
}

#[test]
fn select_updates_keep_display_width_padding_and_field_properties() {
    let mut interp = Interp::new();
    eval(&mut interp, "(require 'eww)");
    assert_eq!(eval(&mut interp, CASES).int(), Some(13));

    // Exercise the shipped source definitions as well as the default AOT load.
    let source = include_str!("../../../lisp/eww.el");
    for (name, next) in [
        ("eww-tag-select", "eww-select-display"),
        ("eww-update-field", "eww-toggle-checkbox"),
    ] {
        let start = source.find(&format!("(defun {name} ")).unwrap();
        let end = source[start..].find(&format!("\n(defun {next} ")).unwrap() + start;
        eval(&mut interp, &source[start..end]);
    }
    assert_eq!(eval(&mut interp, CASES).int(), Some(13));
}
