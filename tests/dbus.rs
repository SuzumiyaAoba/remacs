//! Tests for the D-Bus subrs in src/lisp/builtins/dbus.rs.
//!
//! These tests need two external things:
//!   * libdbus-1 loadable by `dynlib' (feature `dbusbind')
//!   * a reachable session bus ($DBUS_SESSION_BUS_ADDRESS)
//! Without either they return early, so they are no-ops in CI.
//! Expected values were verified against a real `dbus-daemon'
//! session bus (methods on org.freedesktop.DBus).

mod common;
use common::{ev, ev_err};

/// Skip when the D-Bus runtime pieces aren't available.
fn unavailable() -> bool {
    std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err()
        || ev("(featurep 'dbusbind)") != "t"
}

// ---------------------------------------------------------------
// primitives that don't need a live bus
// ---------------------------------------------------------------

#[test]
fn dbus_vars_and_tables() {
    if ev("(featurep 'dbusbind)") != "t" {
        return;
    }
    assert_eq!(ev("(natnump dbus-message-type-invalid)"), "t");
    assert_eq!(ev("dbus-message-type-method-call"), "1");
    assert_eq!(ev("dbus-message-type-method-return"), "2");
    assert_eq!(ev("dbus-message-type-error"), "3");
    assert_eq!(ev("dbus-message-type-signal"), "4");
    assert_eq!(ev("(stringp dbus-runtime-version)"), "t");
    assert_eq!(ev("(stringp dbus-compiled-version)"), "t");
    assert_eq!(ev("(hash-table-p dbus-registered-objects-table)"), "t");
    assert_eq!(ev("(hash-table-p dbus-return-values-table)"), "t");
    assert_eq!(ev("(dbus--registered-fds)"), "nil");
}

#[test]
fn dbus_bad_bus_name() {
    if ev("(featurep 'dbusbind)") != "t" {
        return;
    }
    // GNU: "Wrong bus name" is a dbus-error.
    assert_eq!(ev_err("(dbus--init-bus :bogus)"), "dbus-error");
    assert_eq!(ev_err("(dbus-get-unique-name :bogus)"), "dbus-error");
    // Invalid message type → dbus-error, not a generic error.
    assert_eq!(
        ev_err("(dbus-message-internal 99 :session \"x\")"),
        "dbus-error"
    );
    // Arity: minimum 3 args like GNU's `3, MANY'.
    assert_eq!(
        ev_err("(dbus-message-internal 1 :session)"),
        "wrong-number-of-arguments"
    );
}

#[test]
fn dbus_no_bus_errors() {
    if ev("(featurep 'dbusbind)") != "t" {
        return;
    }
    // Without DBUS_SESSION_BUS_ADDRESS a session connection must not
    // autolaunch — GNU signals "No connection to bus".
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        assert_eq!(ev_err("(dbus--init-bus :session)"), "dbus-error");
        assert_eq!(ev_err("(dbus--init-bus :session-private)"), "dbus-error");
    }
    assert_eq!(ev_err("(dbus-get-unique-name :session-private)"), "dbus-error");
}

// ---------------------------------------------------------------
// live-bus tests (skipped without a session bus)
// ---------------------------------------------------------------

#[test]
fn dbus_init_and_unique_name() {
    if unavailable() {
        return;
    }
    assert_eq!(
        ev("(progn (require 'dbus) (natnump (dbus--init-bus :session)))"),
        "t"
    );
    // Unique names look like ":1.N".
    assert_eq!(
        ev("(progn (require 'dbus) (string-match-p \"\\\\`:[0-9]\" (dbus-get-unique-name :session)))"),
        "0"
    );
    // Private connection (private arg, not the bare *-private keyword)
    // → a *different* unique name registered under :session-private.
    assert_eq!(
        ev("(progn (require 'dbus) (dbus--init-bus :session t) (not (string-equal (dbus-get-unique-name :session) (dbus-get-unique-name :session-private))))"),
        "t"
    );
    // GNU quirk: the *-private keyword alone, without PRIVATE, still
    // opens the *shared* connection (dbus_bus_get) — unique names
    // are equal.
    assert_eq!(
        ev("(progn (require 'dbus) (dbus--init-bus :session-private) (string-equal (dbus-get-unique-name :session) (dbus-get-unique-name :session-private)))"),
        "t"
    );
}

#[test]
fn dbus_call_method_sync() {
    if unavailable() {
        return;
    }
    // GetId → the daemon's GUID string.  A single reply arg is
    // returned unwrapped (GNU `dbus-call-method-handler').
    assert_eq!(
        ev("(progn (require 'dbus) (stringp (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"GetId\")))"),
        "t"
    );
    // GetNameOwner of our own unique name → echoes the unique name.
    assert_eq!(
        ev("(progn (require 'dbus) (let ((u (dbus-get-unique-name :session))) (equal (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"GetNameOwner\" u) u)))"),
        "t"
    );
    // ListNames returns an array whose elements include the daemon.
    assert_eq!(
        ev("(progn (require 'dbus) (member \"org.freedesktop.DBus\" (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"ListNames\")) t)"),
        "t"
    );
    // Method-error reply → dbus-error with the remote error name.
    assert_eq!(
        ev("(progn (require 'dbus) (condition-case e (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"NoSuchMethod\") (dbus-error (nth 1 e))))"),
        "\"org.freedesktop.DBus.Error.UnknownMethod\""
    );
    // Unknown service → ServiceUnknown error.
    assert_eq!(
        ev("(progn (require 'dbus) (condition-case e (dbus-call-method :session \"no.such.Service.Remacs\" \"/x\" \"no.such.If\" \"Meth\") (dbus-error (nth 1 e))))"),
        "\"org.freedesktop.DBus.Error.ServiceUnknown\""
    );
}

#[test]
fn dbus_request_and_release_name() {
    if unavailable() {
        return;
    }
    // RequestName returns a flag (1 = primary owner); single reply
    // arg is unwrapped.
    assert_eq!(
        ev("(progn (require 'dbus) (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"RequestName\" \"org.remacs.test.Name\" 4))"),
        "1"
    );
    // We now own the name.
    assert_eq!(
        ev("(progn (require 'dbus) (equal (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"GetNameOwner\" \"org.remacs.test.Name\") (dbus-get-unique-name :session)))"),
        "t"
    );
    // ReleaseName → 1 (released).
    assert_eq!(
        ev("(progn (require 'dbus) (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"ReleaseName\" \"org.remacs.test.Name\"))"),
        "1"
    );
}

#[test]
fn dbus_signal_registration() {
    if unavailable() {
        return;
    }
    // Register for NameOwnerChanged, own+release a name, and check
    // the signal arrived through `sleep-for' pumping.
    assert_eq!(
        ev("(progn (require 'dbus) (let ((sig nil)) (dbus-register-signal :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"NameOwnerChanged\" (lambda (&rest a) (setq sig a))) (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"RequestName\" \"org.remacs.test.Sig\" 4) (dbus-call-method :session \"org.freedesktop.DBus\" \"/org/freedesktop/DBus\" \"org.freedesktop.DBus\" \"ReleaseName\" \"org.remacs.test.Sig\") (let ((n 0)) (while (and (null sig) (< n 500)) (sleep-for 0.01) (setq n (1+ n)))) (equal (car sig) \"org.remacs.test.Sig\")))"),
        "t"
    );
}
