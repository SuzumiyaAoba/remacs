use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{
        Interp, Value,
        process::{DatagramSocket, ProcIo},
    },
};
use std::time::{Duration, Instant};

#[track_caller]
fn check(i: &mut Interp, frame: &FrameRef, status: &str) {
    let selected = i.selected_frame.as_ref().unwrap().clone();
    let current = i.current_buffer;
    let buffer = frame.borrow().selected.borrow().buffer;
    let name = i
        .buffers
        .get(buffer)
        .unwrap()
        .borrow()
        .name
        .to_unicode_lossy();
    assert!(
        i.eval_str(&format!(
            "(equal (let ((p (get-buffer-process {name:?}))) \
             (if p (symbol-name (process-status p)) \"no process\")) {status:?})"
        ))
        .unwrap()
        .truthy()
    );
    // These string, concatenation, precision and padding results were also
    // checked with GNU 31.1's interactive format-mode-line (integer FACE).
    for (form, expected) in [
        (r#""%s""#, status.to_owned()),
        (
            r#"("" "%b [" "%s" "]|%%s")"#,
            format!("{name} [{status}]|%s"),
        ),
        (r#"(-3 "%s")"#, status.chars().take(3).collect()),
        (r#"(12 "%s")"#, format!("{status:<12}")),
    ] {
        i.eval_str(&format!("(setq-default frame-title-format '{form})"))
            .unwrap();
        assert_eq!(
            render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
            expected,
            "{name}, {status}, {form}"
        );
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &selected
        ));
        assert_eq!(i.current_buffer, current);
    }
}

#[test]
fn frame_titles_follow_their_buffer_process_and_cached_status_transitions() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (setq delete-exited-processes nil)
        (switch-to-buffer (get-buffer-create "process-primary")))"#,
    )
    .unwrap();
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = i
        .eval_str(r#"(make-frame '((window-system . ns) (width . 48) (height . 12)))"#)
        .unwrap()
    else {
        panic!("other frame")
    };
    i.eval_str(
        r#"(progn
        (setq qa-title-other-frame (car (delq (selected-frame) (frame-list))))
        (set-window-buffer (frame-selected-window qa-title-other-frame)
                           (get-buffer-create "process-other"))
        (set-buffer (get-buffer-create "process-unrelated")))"#,
    )
    .unwrap();
    check(&mut i, &primary, "no process");
    check(&mut i, &other, "no process");
    i.eval_str(
        r#"(setq qa-title-pipe
        (make-pipe-process :name "title-pipe" :buffer "process-primary"
                           :noquery t :sentinel #'ignore))"#,
    )
    .unwrap();
    check(&mut i, &primary, "open");
    check(&mut i, &other, "no process");
    i.eval_str("(stop-process qa-title-pipe)").unwrap();
    check(&mut i, &primary, "stop");
    i.eval_str("(continue-process qa-title-pipe)").unwrap();
    check(&mut i, &primary, "open");
    i.eval_str(r#"(set-process-buffer qa-title-pipe (get-buffer "process-other"))"#)
        .unwrap();
    check(&mut i, &primary, "no process");
    check(&mut i, &other, "open");
    i.eval_str("(set-process-buffer qa-title-pipe nil)")
        .unwrap();
    check(&mut i, &other, "no process");
    i.eval_str("(delete-process qa-title-pipe)").unwrap();

    let Value::Process(child) = i
        .eval_str(
            r#"(setq qa-title-child
            (make-process :name "title-child" :buffer "process-primary"
                          :connection-type 'pipe :noquery t
                          :sentinel (lambda (p _event)
                            (setq qa-title-notification
                              (list (process-status p)
                                    (eq p (get-buffer-process (process-buffer p))))))
                          :command '("/usr/bin/true")))"#,
        )
        .unwrap()
    else {
        panic!("child process")
    };
    check(&mut i, &primary, "run");
    check(&mut i, &other, "no process");
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.borrow().status == "run" && Instant::now() < deadline {
        i.eval_str("(accept-process-output qa-title-child 0.01 nil t)")
            .unwrap();
    }
    assert_eq!(child.borrow().status, "exit");
    assert!(matches!(child.borrow().io, ProcIo::None));
    assert!(
        i.eval_str("(equal qa-title-notification '(exit t))")
            .unwrap()
            .truthy()
    );
    // A finished process remains associated while delete-exited-processes
    // is nil; alive() is deliberately not the buffer lookup criterion.
    check(&mut i, &primary, "exit");
    i.eval_str("(delete-process qa-title-child)").unwrap();
    check(&mut i, &primary, "no process");
    // With automatic removal enabled, the sentinel and subsequent titles
    // must see the process removed before the callback is invoked.
    let Value::Process(child) = i
        .eval_str(
            r#"(progn
        (setq delete-exited-processes t qa-title-notification nil)
        (setq qa-title-child
          (make-process :name "title-auto-delete" :buffer "process-primary"
            :connection-type 'pipe :noquery t :command '("/usr/bin/true")
            :sentinel (lambda (p _event)
              (setq qa-title-notification
                (list (process-status p)
                      (eq p (get-buffer-process (process-buffer p)))))))))"#,
        )
        .unwrap()
    else {
        panic!("auto-deleted child process")
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.borrow().status == "run" && Instant::now() < deadline {
        i.eval_str("(accept-process-output qa-title-child 0.01 nil t)")
            .unwrap();
    }
    assert_eq!(child.borrow().status, "exit");
    assert!(matches!(child.borrow().io, ProcIo::None));
    assert!(
        i.eval_str("(equal qa-title-notification '(exit nil))")
            .unwrap()
            .truthy()
    );
    check(&mut i, &primary, "no process");
    // Datagram status notification shares the retention policy. Use only
    // sockets owned by this test and verify the OS socket is released too.
    for automatic in [false, true] {
        let Value::Process(server) = i
            .eval_str(&format!(
                r#"(progn
            (setq delete-exited-processes {} qa-title-notification nil)
            (setq qa-title-udp
              (make-network-process :name "title-udp" :buffer "process-primary"
                :type 'datagram :family 'ipv4 :host "127.0.0.1" :server t
                :service 0 :coding 'binary :noquery t
                :sentinel (lambda (p _event)
                  (setq qa-title-notification
                    (list (process-status p)
                          (eq p (get-buffer-process (process-buffer p)))))))))"#,
                if automatic { "t" } else { "nil" }
            ))
            .unwrap()
        else {
            panic!("datagram process")
        };
        let address = {
            let server = server.borrow();
            let ProcIo::Datagram(io) = &server.io else {
                panic!("datagram transport")
            };
            let DatagramSocket::Inet(socket) = &io.socket else {
                panic!("UDP socket")
            };
            socket.local_addr().unwrap()
        };
        check(&mut i, &primary, "open");
        let peer = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        peer.send_to(b"", address).unwrap();
        i.eval_str("(accept-process-output qa-title-udp 0.1 nil t)")
            .unwrap();
        assert_eq!(server.borrow().status, "closed");
        assert!(matches!(server.borrow().io, ProcIo::None));
        assert!(
            std::net::UdpSocket::bind(address).is_ok(),
            "finished socket released"
        );
        assert!(
            i.eval_str(if automatic {
                "(equal qa-title-notification '(closed nil))"
            } else {
                "(equal qa-title-notification '(closed t))"
            })
            .unwrap()
            .truthy()
        );
        check(
            &mut i,
            &primary,
            if automatic { "no process" } else { "closed" },
        );
        i.eval_str("(delete-process qa-title-udp)").unwrap();
        check(&mut i, &primary, "no process");
    }
}
