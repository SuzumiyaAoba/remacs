//! Regression tests for dlopen-gated optional libraries
//! (src/lisp/dynlib.rs + builtins/{lcms,gnutls,filenotify}.rs) and the
//! image-spec primitives.  Each test skips when its library or OS
//! backend isn't loadable — they are no-ops in minimal environments.

mod common;
use common::{ev, ev_err};

fn lcms2() -> bool {
    ev("(featurep 'lcms2)") == "t"
}
fn gnutls_avail() -> bool {
    ev("(consp (gnutls-available-p))") == "t"
}
fn kqueue() -> bool {
    ev("(featurep 'kqueue)") == "t"
}

// ---------------------------------------------------------------
// LCMS2 (src/lisp/builtins/lcms.rs)
// ---------------------------------------------------------------

#[test]
fn lcms_cie2000_reference() {
    if !lcms2() {
        return;
    }
    // Canonical CIEDE2000 test pair (Sharma et al.) → 2.0425.
    let r = ev("(lcms-cie-de2000 '(50 2.6772 -79.7751) '(50 0.0 -82.7485))");
    let d: f64 = r.parse().unwrap();
    assert!((d - 2.0425).abs() < 1e-3, "cie2000 = {d}");
    assert_eq!(ev("(lcms2-available-p)"), "t");
}

#[test]
fn lcms_white_point_and_roundtrip() {
    if !lcms2() {
        return;
    }
    // D65 white point ≈ (0.9505 1.0 1.0890).
    assert_eq!(
        ev("(lcms-temp->white-point 6500)"),
        "(0.9501657210094707 1.0 1.087653624622009)"
    );
    // xyz -> jch -> jab -> jch -> xyz roughly round-trips.
    assert_eq!(
        ev("(let* ((j (lcms-xyz->jch '(0.5 0.4 0.3))) (x (lcms-jch->xyz (lcms-jab->jch (lcms-jch->jab j))))) (< (abs (- (car x) 0.5)) 0.01))"),
        "t"
    );
    // CAM02-UCS distance is symmetric-ish and non-negative.
    assert_eq!(
        ev("(let ((d (lcms-cam02-ucs '(0.5 0.4 0.3) '(0.4 0.5 0.3)))) (and (numberp d) (>= d 0)))"),
        "t"
    );
    // Bad arg shape → error, not crash.
    assert_eq!(ev_err("(lcms-cie-de2000 '(1 2) '(1 2 3))"), "error");
}

// ---------------------------------------------------------------
// GnuTLS crypto (src/lisp/builtins/gnutls.rs)
// ---------------------------------------------------------------

#[test]
fn gnutls_digests_and_hash() {
    if !gnutls_avail() {
        return;
    }
    assert_eq!(
        ev("(consp (member 'SHA256 (mapcar #'car (gnutls-digests))))"),
        "t"
    );
    // Raw digest bytes → hex = sha256("abc").
    assert_eq!(
        ev("(mapconcat (lambda (c) (format \"%02x\" c)) (gnutls-hash-digest 'SHA256 \"abc\") \"\")"),
        "\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\""
    );
    // HMAC-SHA1(key="key", msg="abc").
    assert_eq!(
        ev("(mapconcat (lambda (c) (format \"%02x\" c)) (gnutls-hash-mac 'SHA1 \"abc\" \"key\") \"\")"),
        "\"ec2270489838611e59c95b51012a92b09d04977e\""
    );
    // Unknown digest name → error.
    assert_eq!(
        ev_err("(gnutls-hash-digest 'NOSUCHDIGEST \"x\")"),
        "error"
    );
}

#[test]
fn gnutls_symmetric_roundtrip() {
    if !gnutls_avail() {
        return;
    }
    // AES-128-CBC encrypt → (ciphertext iv); decrypt round-trips.
    assert_eq!(
        ev("(let* ((r (gnutls-symmetric-encrypt 'AES-128-CBC \"0123456789abcdef\" \"0123456789abcdef\" \"hello12345678901\")) (d (gnutls-symmetric-decrypt 'AES-128-CBC \"0123456789abcdef\" \"0123456789abcdef\" (car r)))) (string= (car d) \"hello12345678901\"))"),
        "t"
    );
    // Result is (ciphertext . actual-iv) list.
    assert_eq!(
        ev("(length (gnutls-symmetric-encrypt 'AES-128-CBC \"0123456789abcdef\" \"0123456789abcdef\" \"hello12345678901\"))"),
        "2"
    );
}

#[test]
fn gnutls_error_helpers() {
    if !gnutls_avail() {
        return;
    }
    // -50 = GNUTLS_E_INVALID_REQUEST → non-fatal, string describes.
    assert_eq!(ev("(stringp (gnutls-error-string -50))"), "t");
    assert_eq!(ev("(gnutls-errorp '(gnutls-error . -50))"), "t");
    // GNU quirk: everything except `t' and `gnutls-e-again' counts.
    assert_eq!(ev("(gnutls-errorp '(other-error . 1))"), "t");
    assert_eq!(ev("(gnutls-errorp t)"), "nil");
    assert_eq!(ev("(gnutls-errorp 'gnutls-e-again)"), "nil");
}

// ---------------------------------------------------------------
// kqueue file notifications (src/lisp/builtins/filenotify.rs)
// ---------------------------------------------------------------

#[test]
fn kqueue_watch_roundtrip() {
    if !kqueue() {
        return;
    }
    // Watch a file, write it, drain via sleep-for → event delivered
    // (event = (DESCRIPTOR ACTION FILE) — ACTION is an atom).
    assert_eq!(
        ev("(progn (require 'filenotify) (let* ((f (make-temp-file \"remacs-kq\")) (evv nil) (desc (file-notify-add-watch f '(change) (lambda (e) (setq evv e))))) (unwind-protect (progn (file-notify-valid-p desc) (write-region \"hello\" nil f) (let ((n 0)) (while (and (null evv) (< n 200)) (sleep-for 0.01) (setq n (1+ n)))) (and evv (consp (memq (nth 1 evv) '(created changed renamed deleted attribute-changed))))) (when desc (file-notify-rm-watch desc)) (delete-file f))))"),
        "t"
    );
    // Removing/validating an unknown descriptor → nil (GNU shape).
    assert_eq!(
        ev("(progn (require 'filenotify) (file-notify-valid-p 99999))"),
        "nil"
    );
}

// ---------------------------------------------------------------
// Image primitives (misc.rs) — GNU tty parity
// ---------------------------------------------------------------

#[test]
fn image_spec_tty_behavior() {
    // Valid spec on a tty frame → "Window system frame should be used".
    assert_eq!(
        ev("(condition-case e (image-size '(image :type xbm :file \"/tmp/x.xbm\")) (error (error-message-string e)))"),
        "\"Window system frame should be used\""
    );
    // image-metadata: nil for invalid spec, window-system error for valid.
    assert_eq!(ev("(image-metadata 'nonsense)"), "nil");
    assert_eq!(
        ev("(condition-case e (image-metadata '(image :type xbm :file \"/tmp/x.xbm\")) (error (error-message-string e)))"),
        "\"Window system frame should be used\""
    );
    // imagep predicate validates spec shape.
    assert_eq!(ev("(imagep '(image :type xbm :file \"f.xbm\"))"), "t");
    assert_eq!(ev("(imagep '(image :type nosuch :file \"f\"))"), "nil");
    assert_eq!(ev("(imagep 'nope)"), "nil");
}

#[test]
fn init_image_library_types() {
    // GNU `lookup_image_type': t for built-in type symbols only.
    assert_eq!(ev("(init-image-library 'xbm)"), "t");
    assert_eq!(ev("(init-image-library 'png)"), "t");
    assert_eq!(ev("(init-image-library 'nosuchtype)"), "nil");
    // Non-symbol arg → nil (GNU compares symbols only).
    assert_eq!(ev("(init-image-library \"png\")"), "nil");
    assert_eq!(ev("(image-type-available-p 'xbm)"), "t");
    assert_eq!(ev("(image-type-available-p 'nosuchtype)"), "nil");
}

#[test]
fn lookup_image_map_geometry() {
    // rect hit-test.
    assert_eq!(
        ev("(cadr (lookup-image-map (list (list (cons 'rect (cons '(0 . 0) '(10 . 10))) 'hit)) 5 5))"),
        "hit"
    );
    assert_eq!(
        ev("(lookup-image-map (list (list (cons 'rect (cons '(0 . 0) '(10 . 10))) 'hit)) 11 5)"),
        "nil"
    );
    // circle.
    assert_eq!(
        ev("(cadr (lookup-image-map (list (list (cons 'circle (cons '(50 . 50) 10)) 'c)) 50 55))"),
        "c"
    );
    // polygon (20x20 square at 20,20).
    assert_eq!(
        ev("(cadr (lookup-image-map (list (list (cons 'poly [20 20 40 20 40 40 20 40]) 'p)) 30 30))"),
        "p"
    );
    assert_eq!(
        ev("(lookup-image-map (list (list (cons 'poly [20 20 40 20 40 40 20 40]) 'p)) 45 30)"),
        "nil"
    );
    // GNU CHECK_FIXNUM on x/y.
    assert_eq!(
        ev_err("(lookup-image-map (list (list '(rect . ((0 . 0) . (1 . 1))) 'r)) \"x\" 1)"),
        "wrong-type-argument"
    );
    assert_eq!(ev("(lookup-image-map nil 0 0)"), "nil");
}
