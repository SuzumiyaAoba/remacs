use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn eval(interp: &mut Interp, source: &str, phase: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{phase}: {} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{phase}: {other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const PRELOAD: &str = r#####"(progn
  ;; Disable Flymake automatic policy before requiring its full source.
  (defvar noninteractive nil)
  (setq noninteractive t)
  (defvar this-command nil)
  (defvar flymake-start-on-flymake-mode nil)
  (defvar flymake-no-changes-timeout nil)
  (defvar flymake-start-on-save-buffer nil)
  (defvar flymake-diagnostic-functions nil)
  (defvar flymake-mode-hook nil)
  (defvar flymake-timer nil)
  (defvar flymake-indicator-type nil)
  (defvar flymake-autoresize-margins nil)
  (defvar flymake-show-diagnostics-at-end-of-line nil)
  (setq flymake-start-on-flymake-mode nil
        flymake-no-changes-timeout nil
        flymake-start-on-save-buffer nil
        flymake-diagnostic-functions nil
        flymake-mode-hook nil
        flymake-timer nil
        flymake-indicator-type nil
        flymake-autoresize-margins nil
        flymake-show-diagnostics-at-end-of-line nil)
  ;; The spy returns a valid inactive timer, never an activated one.
  (require 'timer)
  (defvar remacs-flymake-b17-idle-timer-calls 0)
  (defvar remacs-flymake-b17-idle-timer-args nil)
  (defvar remacs-flymake-b17-idle-timer-object nil)
  (defvar remacs-flymake-b17-old-run-with-idle-timer nil)
  (defvar remacs-flymake-b17-had-run-with-idle-timer nil)
  (defun remacs-flymake-b17-idle-timer-spy (&rest args)
    (setq remacs-flymake-b17-idle-timer-calls
          (1+ remacs-flymake-b17-idle-timer-calls)
          remacs-flymake-b17-idle-timer-args args
          remacs-flymake-b17-idle-timer-object (timer-create)))
  (setq remacs-flymake-b17-had-run-with-idle-timer
        (fboundp 'run-with-idle-timer)
        remacs-flymake-b17-old-run-with-idle-timer
        (and remacs-flymake-b17-had-run-with-idle-timer
             (symbol-function 'run-with-idle-timer)))
  (fset 'run-with-idle-timer #'remacs-flymake-b17-idle-timer-spy)
  t)"#####;

const LIFECYCLE: &str = r#####"(progn
  ;; The outer unwind-protect includes REQUIRE_FORM so the PRELOAD timer spy
  ;; is restored even when loading Flymake signals.
  (unwind-protect
      (progn
        (unless (not (featurep 'flymake))
          (error "Fresh interpreter unexpectedly already provided flymake"))
        (let ((load-path (cons LISP_DIR load-path)))
          (require 'flymake))
        (unless (and (fboundp 'flymake-mode)
                     (fboundp 'flymake-start)
                     (fboundp 'flymake-diagnostics))
          (error "Full Flymake API was not loaded"))
        (unless (commandp 'flymake-start)
          (error "flymake-start is not a public command"))
        (unless (zerop remacs-flymake-b17-idle-timer-calls)
          (error "Loading Flymake attempted to register an idle timer"))

        (defvar remacs-flymake-b17-backend-calls 0)
        (defun remacs-flymake-b17-assert (condition message)
          (unless condition (error "%s" message)))
        (defun remacs-flymake-b17-toy-backend (report-fn &rest _args)
          (setq remacs-flymake-b17-backend-calls
                (1+ remacs-flymake-b17-backend-calls))
          (funcall report-fn
                   (list (flymake-make-diagnostic
                          (current-buffer) 1 2 :error
                          (format "b17 toy diagnostic %s" remacs-flymake-b17-backend-calls)))))
        (put 'remacs-flymake-b17-toy-backend 'flymake-always-safe t)

        (let ((flymake-start-on-flymake-mode nil)
              (flymake-no-changes-timeout nil)
              (flymake-start-on-save-buffer nil)
              (flymake-mode-hook nil)
              (flymake-indicator-type nil)
              (flymake-autoresize-margins nil)
              (flymake-show-diagnostics-at-end-of-line nil)
              (this-command nil)
              (remacs-flymake-b17-backend-calls 0))
          (let ((original-buffer (current-buffer))
                (buffer (get-buffer-create "*flymake-b17-lifecycle*")))
            (unwind-protect
                (progn
                  (switch-to-buffer buffer)
                  (with-current-buffer buffer
                    (erase-buffer)
                    (insert "abc")
                    ;; Isolate hook variables before mode activation.  The
                    ;; checks below use the actual local lists mode installs.
                    (setq-local after-change-functions nil)
                    (setq-local after-save-hook nil)
                    (setq-local flymake-diagnostic-functions nil)
                    (add-hook 'flymake-diagnostic-functions
                              #'remacs-flymake-b17-toy-backend nil t)
                    (setq-local flymake-diagnostic-functions
                                '(remacs-flymake-b17-toy-backend))
                    (remacs-flymake-b17-assert
                     (equal flymake-diagnostic-functions
                            '(remacs-flymake-b17-toy-backend))
                     "The fixture did not isolate the local toy backend")

                    ;; #2261: actual mode activation installs the monitoring
                    ;; hooks while automatic start/idle/save are disabled.
                    (flymake-mode 1)
                    (remacs-flymake-b17-assert flymake-mode
                                               "Flymake did not enable")
                    (remacs-flymake-b17-assert
                     (and (memq 'flymake-after-change-function after-change-functions)
                          (memq 'flymake-after-save-hook after-save-hook))
                     "Flymake did not register the actual change/save hooks")
                    (remacs-flymake-b17-assert
                     (zerop remacs-flymake-b17-backend-calls)
                     "Mode activation automatically ran the backend")

                    ;; A real edit invokes after-change, but nil timeout stops
                    ;; scheduling and does not run the backend.
                    (goto-char (point-max))
                    (insert "d")
                    (remacs-flymake-b17-assert
                     (and (zerop remacs-flymake-b17-backend-calls)
                          (zerop remacs-flymake-b17-idle-timer-calls))
                     "Disabled idle auto-check ran a backend or requested a timer")
                    ;; Invoke the actual local after-save-hook list, without
                    ;; saving a file, while start-on-save is disabled.
                    (run-hooks 'after-save-hook)
                    (remacs-flymake-b17-assert
                     (zerop remacs-flymake-b17-backend-calls)
                     "Disabled save auto-check ran the backend")

                    ;; #2266: toggle turns mode off and removes its local hooks.
                    (flymake-mode 'toggle)
                    (remacs-flymake-b17-assert
                     (not flymake-mode)
                     "flymake-mode 'toggle did not disable Flymake")
                    (remacs-flymake-b17-assert
                     (and (not (memq 'flymake-after-change-function after-change-functions))
                          (not (memq 'flymake-after-save-hook after-save-hook)))
                     "Mode-off did not remove its actual change/save hooks")
                    (remacs-flymake-b17-assert
                     (zerop remacs-flymake-b17-backend-calls)
                     "Toggle-off unexpectedly ran a backend")

                    ;; Re-enable, still without auto-start, and test #2265's
                    ;; explicit public command using the sole local backend.
                    (flymake-mode 1)
                    (remacs-flymake-b17-assert
                     (and flymake-mode
                          (memq 'flymake-after-change-function after-change-functions)
                          (memq 'flymake-after-save-hook after-save-hook)
                          (zerop remacs-flymake-b17-backend-calls))
                     "Re-enable changed hooks or auto-started a backend")
                    (flymake-start)
                    (remacs-flymake-b17-assert
                     (= remacs-flymake-b17-backend-calls 1)
                     "Explicit flymake-start did not run the toy backend once")
                    (remacs-flymake-b17-assert
                     (zerop remacs-flymake-b17-idle-timer-calls)
                     "Explicit flymake-start unexpectedly requested an idle timer")
                    (remacs-flymake-b17-assert
                     (let ((diags (flymake-diagnostics)))
                       (and (= (length diags) 1)
                            (let ((diag (car diags)))
                              (and (eq (flymake-diagnostic-type diag) :error)
                                   (equal (flymake-diagnostic-message diag)
                                          "b17 toy diagnostic 1")
                                   (= (flymake-diagnostic-beg diag) 1)
                                   (= (flymake-diagnostic-end diag) 2)
                                   (eq (flymake-diagnostic-buffer diag) buffer)))))
                     "Explicit start did not publish the toy diagnostic")

                    ;; Positive idle path: capture, but never activate, the
                    ;; actual timer callback and its arguments.
                    (setq flymake-no-changes-timeout 0.25)
                    (goto-char (point-max))
                    (insert "e")
                    (remacs-flymake-b17-assert
                     (and (= remacs-flymake-b17-idle-timer-calls 1)
                          (= remacs-flymake-b17-backend-calls 1)
                          (eq flymake-timer remacs-flymake-b17-idle-timer-object))
                     "after-change did not schedule exactly one idle callback")
                    (let ((args remacs-flymake-b17-idle-timer-args))
                      (remacs-flymake-b17-assert
                       (and (= (length args) 4)
                            (equal (nth 0 args) (seconds-to-time 0.25))
                            (null (nth 1 args))
                            (functionp (nth 2 args))
                            (eq (nth 3 args) buffer))
                       (format "Idle callback arguments changed: %S" args))
                      (let ((this-command nil))
                        (funcall (nth 2 args) (nth 3 args))))
                    (remacs-flymake-b17-assert
                     (and (= remacs-flymake-b17-backend-calls 2)
                          (null flymake-timer))
                     "Manual idle callback did not start a check and clear its timer")
                    (remacs-flymake-b17-assert
                     (equal (mapcar #'flymake-diagnostic-message (flymake-diagnostics))
                            '("b17 toy diagnostic 2"))
                     "Idle callback did not publish its diagnostic")

                    ;; Positive after-save path uses the hook list registered
                    ;; by Flymake.  No file is actually saved.
                    (setq flymake-no-changes-timeout nil
                          flymake-start-on-save-buffer t)
                    (let ((this-command nil))
                      (run-hooks 'after-save-hook))
                    (remacs-flymake-b17-assert
                     (= remacs-flymake-b17-backend-calls 3)
                     "after-save-hook did not start its additional check")
                    (remacs-flymake-b17-assert
                     (and (= remacs-flymake-b17-idle-timer-calls 1)
                          (null flymake-timer)
                          (equal (mapcar #'flymake-diagnostic-message (flymake-diagnostics))
                            '("b17 toy diagnostic 3")))
                     "after-save path scheduled a timer or lost its diagnostic"))
                  t)
              (with-current-buffer buffer
                (when flymake-mode (flymake-mode -1))
                (when flymake-timer (cancel-timer flymake-timer))
                (set-buffer-modified-p nil))
              (when (buffer-live-p buffer) (kill-buffer buffer))
              (when (buffer-live-p original-buffer)
                (switch-to-buffer original-buffer))))
        t))
    (progn
      (when flymake-timer (cancel-timer flymake-timer))
      (if remacs-flymake-b17-had-run-with-idle-timer
          (fset 'run-with-idle-timer remacs-flymake-b17-old-run-with-idle-timer)
        (fmakunbound 'run-with-idle-timer))))
  t)"#####;

const FORMATTER: &str = r#####"(progn
  (require 'flymake)
  (defun remacs-flymake-text-assert (condition message)
    (unless condition (error "%s" message)))
  (let* ((plain (flymake-make-diagnostic
                 (current-buffer) 1 2 :error "plain"))
         (origin-only (flymake-make-diagnostic
                       (current-buffer) 1 2 :error '("backend" nil "msg")))
         (numeric-code (flymake-make-diagnostic
                        (current-buffer) 1 2 :error '(nil 42 "bad")))
         (full (flymake-make-diagnostic
                (current-buffer) 1 2 :error '("backend" 42 "bad")))
         (multiline (flymake-make-diagnostic
                     (current-buffer) 1 2 :error
                     '("backend" 42 "first line\nsecond line")))
         (empty-origin (flymake-make-diagnostic
                        (current-buffer) 1 2 :error '("" nil "msg"))))
    ;; #2251: omitted/default PARTS and explicit default parts have no leading
    ;; separators; origin attaches the colon without a preceding space.
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text plain) "plain")
     "default parts inserted separators for missing origin and code")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text plain '(origin code message)) "plain")
     "explicit default parts inserted leading separators")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text origin-only) "backend: msg")
     "origin/message separator contains whitespace before the colon")
    ;; #2361: PARTS controls inclusion, never canonical output order.
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text full '(message origin code))
            "backend [42]: bad")
     "PARTS order replaced canonical origin/code/message order")
    ;; #2362: omitted PARTS defaults, explicit nil selects nothing.
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text full nil) "")
     "explicit nil PARTS was treated as omitted")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text full '()) "")
     "empty PARTS did not produce the empty string")
    ;; Numeric code is normalized to a string by flymake-make-diagnostic.
    (remacs-flymake-text-assert
     (and (equal (flymake-diagnostic-code numeric-code) "42")
          (equal (flymake-diagnostic-text numeric-code '(code message))
                 "[42]: bad"))
     "numeric diagnostic code formatting changed")
    ;; Repeated and unknown selectors do not duplicate output or add spaces.
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text full
                                    '(message unknown origin message code origin))
            "backend [42]: bad")
     "duplicate/unknown PARTS changed canonical formatting")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text full '(unknown)) "")
     "unknown-only PARTS did not return an empty string")
    ;; oneliner is used only when message is not requested; its position in
    ;; PARTS still does not affect canonical field order.
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text multiline '(oneliner origin code))
            "backend [42]: first line")
     "oneliner did not truncate at the first newline")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text multiline '(oneliner message))
            "first line\nsecond line")
     "message-only PARTS unexpectedly included origin or code")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text multiline
                                    '(origin code oneliner message))
            "backend [42]: first line\nsecond line")
     "message did not take precedence over oneliner with origin/code selected")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text plain '(oneliner)) "plain")
     "oneliner without a newline did not preserve the whole message")
    (remacs-flymake-text-assert
     (equal (flymake-diagnostic-text empty-origin '(origin message)) ": msg")
     "empty origin whitespace did not follow GNU concat rules"))
  t)"#####;

const REGION: &str = r#####"(progn
  (unless (fboundp 'flymake-diag-region)
    (error "flymake.el source was not loaded"))
  (defun remacs-flymake-b17-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-flymake-b17-region (text line col)
    (with-temp-buffer
      (insert text)
      (flymake-diag-region (current-buffer) line col)))
  (defun remacs-flymake-b17-check (label text line col expected)
    (let ((actual (remacs-flymake-b17-region text line col)))
      (remacs-flymake-b17-assert
       (equal actual expected)
       (format "Flymake region %S: %S line=%S col=%S => %S expected %S"
               label text line col actual expected))))

  ;; #2253: the nil-column region starts after indentation and trims trailing
  ;; horizontal whitespace; positive columns use the containing sexp/symbol.
  (remacs-flymake-b17-check '2253-line "  alpha   \n" 1 nil '(3 . 8))
  (remacs-flymake-b17-check '2253-indent-column "  alpha   \n" 1 1 '(1 . 2))
  (remacs-flymake-b17-check '2253-word-column "  alpha   \n" 1 3 '(3 . 8))

  ;; #2363: nonpositive COL takes the same fallback as nil, never an
  ;; out-of-buffer position.
  (remacs-flymake-b17-check '2363-col-zero "abc  \n" 1 0 '(1 . 4))
  (remacs-flymake-b17-check '2363-col-negative "abc  \n" 1 -1 '(1 . 4))

  ;; Negative LINE is clamped to line 1; a too-large LINE is clamped to the
  ;; last absolute line in the accessible buffer.
  (remacs-flymake-b17-check 'negative-line "  alpha   \n" -1 nil '(3 . 8))
  (remacs-flymake-b17-check 'line-past-end "first\n  last   " 999 nil '(9 . 13))

  ;; Blank and whitespace-only lines follow GNU's fallback endpoint rules.
  (remacs-flymake-b17-check 'empty-buffer "" 1 nil '(1 . 1))
  (remacs-flymake-b17-check 'empty-line "\nnext\n" 1 nil '(1 . 2))
  (remacs-flymake-b17-check 'whitespace-line "  \nnext\n" 1 nil '(3 . 4))
  (remacs-flymake-b17-check 'whitespace-eob "first\n   " 2 nil '(1 . 6))

  ;; A positive column beyond this line but still inside the buffer is applied
  ;; from the requested line start, matching GNU's forward-char behavior.
  (remacs-flymake-b17-check 'column-into-next-line "abc\nxyz\n" 1 5 '(5 . 8))
  ;; A column beyond the accessible buffer fails to form a region.
  (remacs-flymake-b17-assert
   (null (remacs-flymake-b17-region "abc" 1 99))
   "Out-of-buffer column did not return nil")

  ;; Narrowing is preserved; line numbers remain absolute, while positions
  ;; stay within the current restriction.
  (with-temp-buffer
    (insert "prefix\n  alpha   \nsuffix\n")
    (narrow-to-region 8 18)
    (goto-char 15)
    (re-search-backward "alpha" nil t)
    (let ((before-point (point))
          (before-min (point-min))
          (before-max (point-max))
          (before-match (match-data)))
      (let ((region (flymake-diag-region (current-buffer) 1 nil)))
        (remacs-flymake-b17-assert
         (equal region '(10 . 15))
         (format "Narrowed region changed: %S" region)))
      (remacs-flymake-b17-assert
       (and (= (point) before-point)
            (= (point-min) before-min)
            (= (point-max) before-max))
       "diag-region moved point or changed narrowing")
      (remacs-flymake-b17-assert
       (equal (match-data) before-match)
       "diag-region did not restore match data")))

  ;; BUFFER selection is scoped with-current-buffer; callers stay in their
  ;; original buffer after querying another buffer.
  (let ((target (get-buffer-create "*flymake-b17-target*")))
    (with-current-buffer target
      (erase-buffer)
      (insert "  alpha   \n")
      (set-buffer-modified-p nil))
    (with-temp-buffer
      (let ((caller (current-buffer))
            (region (flymake-diag-region target 1 nil)))
        (remacs-flymake-b17-assert
         (equal region '(3 . 8))
         (format "Other-buffer query returned %S" region))
        (remacs-flymake-b17-assert
         (eq (current-buffer) caller)
         "Other-buffer query changed the caller's current buffer")))
    (kill-buffer target))

  ;; Buffer positions count characters, including multibyte text, not bytes.
  (remacs-flymake-b17-check 'unicode-column "界x\n" 1 2 '(2 . 3))

  ;; A positive explicit column still highlights a complete sexp where one
  ;; exists, rather than reducing the requested span to one character.
  (remacs-flymake-b17-check 'sexp-column "alpha beta\n" 1 1 '(1 . 6))
  t)"#####;

const SURFACE: &str = r#####"(progn
  (defvar remacs-flymake-b17-hook-states nil)
  (defun remacs-flymake-b17-mode-hook ()
    (push flymake-mode remacs-flymake-b17-hook-states))
  (require 'flymake)
  (defun remacs-flymake-b17-assert (condition message)
    (unless condition (error "%s" message)))
  ;; #2252: text properties are part of the API result, not just its string.
  (with-temp-buffer
    (let ((diag (flymake-make-diagnostic
                 (current-buffer) 1 2 :warning "bad")))
      (let ((painted (flymake-diagnostic-oneliner diag))
            (plain (flymake-diagnostic-oneliner diag t)))
        (remacs-flymake-b17-assert
         (eq (get-text-property 0 'face painted) 'flymake-warning-echo)
         "default oneliner omitted the warning echo face")
        (remacs-flymake-b17-assert
         (null (get-text-property 0 'face plain))
         "NOPAINTP retained the echo face")
        (remacs-flymake-b17-assert
         (and (equal painted "bad") (equal plain "bad"))
         "oneliner changed diagnostic text"))))
  ;; #2258/#2260: the minor-mode declaration registers a lighter, and its
  ;; ordinary hook runs after enabling and disabling.  Keep backend/timer
  ;; activation off for this API-only probe.
  (let ((flymake-start-on-flymake-mode nil)
        (flymake-no-changes-timeout nil)
        (flymake-diagnostic-functions nil)
        (flymake-indicator-type nil))
    (with-temp-buffer
      (setq remacs-flymake-b17-hook-states nil)
      (add-hook 'flymake-mode-hook #'remacs-flymake-b17-mode-hook nil t)
      (let ((enable-result (flymake-mode 1)))
        (remacs-flymake-b17-assert (eq enable-result t)
                                   "mode enable did not return t")
        (remacs-flymake-b17-assert
         (equal (assq 'flymake-mode minor-mode-alist)
                '(flymake-mode flymake-mode-line-format))
         "Flymake lighter was not registered in minor-mode-alist")
        (remacs-flymake-b17-assert
         (equal remacs-flymake-b17-hook-states '(t))
         "Flymake mode hook did not run on enable"))
      (let ((disable-result (flymake-mode -1)))
        (remacs-flymake-b17-assert (null disable-result)
                                   "mode disable did not return nil")
        (remacs-flymake-b17-assert (not flymake-mode)
                                   "Flymake mode remained enabled")
        (remacs-flymake-b17-assert
         (equal remacs-flymake-b17-hook-states '(nil t))
         "Flymake mode hook did not run on disable"))))
  t)"#####;

#[test]
fn gnu_flymake_public_api_and_local_backend() {
    let mut interp = Interp::new_with_noninteractive(true);
    assert_eq!(eval(&mut interp, "(featurep 'flymake)", "fresh state"), "nil");
    assert_eq!(eval(&mut interp, PRELOAD, "preload"), "t");
    let lisp = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp");
    let lifecycle = LIFECYCLE.replace("LISP_DIR", &lisp_string(lisp.to_str().unwrap()));
    assert_eq!(eval(&mut interp, &lifecycle, "lifecycle"), "t");
    for (phase, cases) in [
        ("formatter", FORMATTER),
        ("region", REGION),
        ("surface", SURFACE),
    ] {
        assert_eq!(eval(&mut interp, cases, phase), "t", "{phase}");
    }
}
