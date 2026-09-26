//! Regression tests for the NeXTstep/macOS primitives
//! (src/lisp/builtins/nsgui.rs) and for the platform Lisp libraries
//! (term/ns-win.el, term/common-win.el, fontset.el) that GNU dumps
//! into the image.  Everything here targets batch/TTY behaviour:
//! there is no NS terminal, so GUI entry points must either work
//! headlessly (font-name, colours, accessibility, sleep assertions)
//! or signal GNU's exact errors.  The whole file is macOS-gated —
//! other platforms lack libobjc/AppKit and provide none of these.
#![cfg(target_os = "macos")]

mod common;
use common::{ev, ev_err};

/// Interp::new pays the prelude cost once per `ev', so each test
/// evaluates one big `let'/`dolist' probe instead of many forms.
#[test]
fn startup_platform_lisp_defs() {
    // term/ns-win.el, term/common-win.el and fontset.el are
    // pre-registered features whose real definitions must still be
    // evaluated during Interp::new — GNU's -Q has all of these bound.
    assert_eq!(
        ev("(list (fboundp 'ns-ignore-1-arg) (fboundp 'ns-parse-geometry)
                  (fboundp 'ns-handle-nxopen) (fboundp 'ns-handle-nxopentemp)
                  (fboundp 'x-handle-args) (fboundp 'x-handle-geometry)
                  (fboundp 'x-compose-font-name) (fboundp 'x-decompose-font-name)
                  (fboundp 'ns-define-service) (fboundp 'ns-spi-service-call)
                  (fboundp 'ns-insert-working-text) (fboundp 'ns-in-echo-area)
                  (fboundp 'ns-open-file-select-line) (fboundp 'ns-unselect-line)
                  (fboundp 'ns-drag-n-drop) (fboundp 'ns-handle-drag-motion)
                  (fboundp 'x-file-dialog) (fboundp 'x-setup-function-keys)
                  (boundp 'ns-input-file) (boundp 'ns-version-string)
                  (boundp 'ns-initialized) (boundp 'ns-alternate-modifier))"),
        "(t t t t t t t t t t t t t t t t t t t t t t)"
    );
    // GNU -Q leaves these unbound (nsterm creates them lazily).
    assert_eq!(
        ev("(list (boundp 'ns-selection-colors) (boundp 'x-invocation-args))"),
        "(nil nil)"
    );
    // ns-ignore-1-arg pops x-invocation-args (dynamic var — setq,
    // not let, so the binding is visible inside the function under
    // lexical evaluation).
    assert_eq!(
        ev("(progn (setq x-invocation-args '(\"emacs\" \"-x\" \"-y\"))
                  (ns-ignore-1-arg \"-x\")
                  (prog1 x-invocation-args
                    (makunbound 'x-invocation-args)))"),
        "(\"-x\" \"-y\")"
    );
    // Nextstep geometry: `top left height width'.
    assert_eq!(
        ev("(ns-parse-geometry \"80 100 30 50\")"),
        "((top . 80) (left . 100) (height . 30) (width . 50))"
    );
    // fontset.el round-trip.
    assert_eq!(
        ev("(x-decompose-font-name \"-*-fixed-medium-r-normal-*-13-120-75-75-c-70-iso8859-1\")"),
        "[\"*-fixed\" \"medium\" \"r\" \"normal\" nil \"13\" \"120\" \"75\" \"75\" \"c\" \"70\" \"iso8859-1\"]"
    );
}

#[test]
fn headless_queries() {
    // ns-font-name resolves simple names and passes XLFD through.
    assert_eq!(ev("(ns-font-name \"Monaco\")"), "\"Monaco\"");
    assert_eq!(
        ev("(ns-font-name \"fontset-default\")"),
        "\"fontset-default\""
    );
    assert_eq!(ev_err("(ns-font-name 42)"), "wrong-type-argument");
    // Accessibility trust is a plain boolean.
    assert_eq!(
        ev("(or (eq t (ns-process-is-accessibility-trusted))
                (eq nil (ns-process-is-accessibility-trusted)))"),
        "t"
    );
    // NSColorList gives several hundred names, all strings.
    assert_eq!(
        ev("(let ((c (ns-list-colors)))
             (and (> (length c) 100)
                  (cl-every 'stringp c)
                  (member \"red\" c) t))"),
        "t"
    );
    // Optional FRAME arg must be a frame — GNU checks FRAMEP.
    assert_eq!(ev_err("(ns-list-colors \"Apple\")"), "wrong-type-argument");
}

#[test]
fn sleep_assertion_roundtrip() {
    // beginActivityWithOptions: token is a fixnum, endActivity: t;
    // the token must survive its autorelease pool (used to crash).
    assert_eq!(
        ev("(let ((tok (ns-block-system-sleep \"test\" nil)))
             (and (integerp tok)
                  (eq (ns-unblock-system-sleep tok) t)
                  (null (ns-unblock-system-sleep 999))))"),
        "t"
    );
}

#[test]
fn window_system_errors_and_nils() {
    // GNU NS build, no NS terminal: correct-arity calls signal the
    // exact "Window system is not in use or not initialized" text.
    for form in [
        "(ns-hide-others)",
        "(ns-emacs-info-panel)",
        "(ns-show-character-palette)",
        "(ns-popup-color-panel)",
        "(ns-mouse-absolute-pixel-position)",
        "(ns-frame-geometry)",
        "(ns-frame-edges)",
        "(ns-set-mouse-absolute-pixel-position 0 0)",
        "(ns-frame-restack nil nil)",
        "(ns-selection-owner-p 'PRIMARY)",
        "(ns-own-selection-internal 'PRIMARY \"x\")",
        "(ns-get-selection 'PRIMARY 'STRING)",
        "(ns-disown-selection-internal 'PRIMARY)",
        "(ns-perform-service \"x\" nil)",
        "(ns-do-applescript \"beep\")",
        "(ns-read-file-name \"x\")",
        "(x-file-dialog \"p\" nil)",
        "(x-apply-session-resources)",
        "(ns-get-resource \"a\" \"b\")",
        "(ns-hide-emacs nil)",
        "(ns-set-resource \"a\" \"b\" \"c\")",
        "(ns-begin-drag '(a) 'b nil)",
    ] {
        let r = ev(&format!(
            "(condition-case e {form} (error (car (cdr e))))"
        ));
        assert_eq!(
            r,
            "\"Window system is not in use or not initialized\"",
            "{form}"
        );
    }
    // These legitimately return nil without a window system.
    assert_eq!(
        ev("(list (ns-badge nil) (ns-request-user-attention nil)
                  (ns-progress-indicator 0.5) (ns-in-echo-area)
                  (ns-unselect-line) (ns-selection-exists-p 'PRIMARY)
                  (ns-list-services) (ns-reset-menu)
                  (ns-frame-list-z-order) (ns-display-monitor-attributes-list)
                  (x-handle-named-frame-geometry nil))"),
        "(nil nil nil nil nil nil nil nil nil nil nil)"
    );
    // x-create-frame/x-select-font carry Nextstep-specific messages.
    assert_eq!(
        ev("(condition-case e (x-create-frame '((visibility)))
             (error (car (cdr e))))"),
        "\"Nextstep windows are not in use or not initialized\""
    );
    assert_eq!(
        ev("(condition-case e (x-select-font)
             (error (car (cdr e))))"),
        "\"Window system frame should be used\""
    );
    // x-begin-drag consults the XdndSelection local binding first —
    // GNU's own error text.
    assert_eq!(
        ev("(condition-case e (x-begin-drag '(a) 'b)
             (error (car (cdr e))))"),
        "\"No local value for XdndSelection\""
    );
}
