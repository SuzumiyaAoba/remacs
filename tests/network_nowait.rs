//! Kernel evidence for #179: :nowait TCP must really use nonblocking connect.
//! All endpoints are self-owned loopback sockets; no public service, process
//! signal, fixed port, or unbounded fixture wait is used.
mod common;

use common::*;
use remacs::lisp::process::ProcIo;
use remacs::lisp::{Interp, Value};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::{Duration, Instant};

// Descriptor-close assertions must not race another test reusing the fd.
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|e| panic!("{source}: {e:?}"))
}

fn printed(i: &mut Interp, source: &str) -> String {
    let value = eval(i, source);
    i.prin1_to_string(&value)
}

fn setup(i: &mut Interp) {
    i.noninteractive = true;
    eval(
        i,
        r#"(setq text-quoting-style 'grave print-escape-newlines t)
        (defun nw-record (p event)
          (process-put p 'events (append (process-get p 'events) (list event))))
        (defun nw-sentinel (p event)
          (nw-record p (list 'sentinel event (process-status p)
                              (and (process-live-p p) t))))
        (defun nw-filter (p text)
          (nw-record p (list 'filter (process-status p)))
          (process-put p 'received (concat (process-get p 'received) text)))"#,
    );
}

fn connect(i: &mut Interp, address: SocketAddr, options: &str) -> Value {
    eval(
        i,
        &format!(
            "(setq nw-client (make-network-process :name \"owned-nowait\" \
             :host {:?} :family '{} :service {} :nowait t :coding 'binary \
             :noquery t :filter #'nw-filter {options}))",
            address.ip().to_string(),
            if address.is_ipv4() { "ipv4" } else { "ipv6" },
            address.port()
        ),
    )
}

fn pending_fd(process: &Value) -> RawFd {
    let Value::Process(p) = process else {
        panic!("expected network process");
    };
    let p = p.borrow();
    let ProcIo::Connecting(connect) = &p.io else {
        panic!("initial nowait backend must represent an unfinished connect");
    };
    connect.as_raw_fd()
}

fn assert_nonblocking(fd: RawFd) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(flags >= 0, "pending descriptor must be open");
    assert_ne!(
        flags & libc::O_NONBLOCK,
        0,
        "connect must be kernel-nonblocking"
    );
}

fn assert_closed(fd: RawFd) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

fn wait_for(i: &mut Interp, predicate: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while printed(i, predicate) == "nil" && Instant::now() < deadline {
        eval(i, "(accept-process-output nil 0.01)");
    }
    assert_eq!(printed(i, predicate), "t", "timed out: {predicate}");
}

fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // BSD accept inherits the listener's nonblocking flag;
                // clear it so the peer's I/O timeouts actually wait.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                return stream;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "owned listener accept timed out");
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => panic!("owned listener accept: {e}"),
        }
    }
}

#[test]
fn nowait_kernel_success_orders_open_before_ready_input_and_early_send() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        setup(&mut i);
        let mut listeners = vec![TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap()];
        if let Ok(v6) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
            listeners.push(v6);
        }
        for listener in listeners {
            let address = listener.local_addr().unwrap();
            let process = connect(
                &mut i,
                address,
                r#":sentinel (lambda (p event)
                   (nw-sentinel p event)
                   (when (equal event "open\n")
                     (accept-process-output p 0.02)
                     (process-put p 'during-open (process-get p 'received))))"#,
            );
            let fd = pending_fd(&process);
            assert_nonblocking(fd);
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
            assert_eq!(printed(&mut i, "(process-get nw-client 'events)"), "nil");
            let mut peer = accept(&listener);
            // Data is already ready in the kernel before completion is polled.
            peer.write_all(b"already-ready").unwrap();
            wait_for(
                &mut i,
                "(equal (process-get nw-client 'received) \"already-ready\")",
            );
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'during-open)"),
                "nil"
            );
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'events)"),
                "((sentinel \"open\\n\" open t) (filter open))"
            );
            assert_eq!(printed(&mut i, "(process-exit-status nw-client)"), "0");
            let Value::Process(p) = &process else {
                unreachable!()
            };
            {
                let p = p.borrow();
                let ProcIo::Net(stream) = &p.io else {
                    panic!("successful connect must become the existing TCP backend");
                };
                assert_eq!(stream.peer_addr().unwrap(), address);
                assert_eq!(stream.as_raw_fd(), fd, "completion must keep the socket");
                assert_nonblocking(stream.as_raw_fd());
            }
            eval(&mut i, "(delete-process nw-client)");
            assert_closed(fd);
            drop(peer);

            // Deleting from the open sentinel must suppress bytes that were
            // already ready. A replacement sentinel sees deletion immediately.
            let process = connect(
                &mut i,
                address,
                r#":sentinel (lambda (p event)
                    (nw-sentinel p event)
                    (when (equal event "open\n")
                      (set-process-sentinel p (lambda (proc message)
                        (nw-record proc (list 'replacement message (process-status proc)))))
                      (delete-process p)))"#,
            );
            let fd = pending_fd(&process);
            let mut peer = accept(&listener);
            peer.write_all(b"must-not-deliver").unwrap();
            wait_for(&mut i, "(eq (process-status nw-client) 'closed)");
            assert_closed(fd);
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'events)"),
                "((sentinel \"open\\n\" open t) (replacement \"deleted\\n\" closed))"
            );
            assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");
            eval(&mut i, "(accept-process-output nil 0.02)");
            assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");
            drop(peer);

            // The open sentinel can inhibit input after bytes are already
            // queued in the kernel; restoring a real filter must recover them.
            let process = connect(
                &mut i,
                address,
                r#":sentinel (lambda (p event)
                    (nw-sentinel p event)
                    (when (equal event "open\n") (set-process-filter p t)))"#,
            );
            let fd = pending_fd(&process);
            let mut peer = accept(&listener);
            peer.write_all(b"held-by-filter").unwrap();
            wait_for(&mut i, "(eq (process-status nw-client) 'open)");
            eval(&mut i, "(accept-process-output nil 0.03)");
            assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'events)"),
                "((sentinel \"open\\n\" open t))"
            );
            eval(&mut i, "(set-process-filter nw-client #'nw-filter)");
            wait_for(
                &mut i,
                "(equal (process-get nw-client 'received) \"held-by-filter\")",
            );
            eval(&mut i, "(delete-process nw-client)");
            assert_closed(fd);
            drop(peer);

            // :stop suppresses reads, not connection establishment or sends.
            // This also catches treating the stopped Connecting backend as Net.
            let process = connect(&mut i, address, ":stop t :sentinel #'nw-sentinel");
            let fd = pending_fd(&process);
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "stop");
            assert_eq!(
                printed(&mut i, "(process-send-string nw-client \"stopped\")"),
                "nil"
            );
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "stop");
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'events)"),
                "((sentinel \"open\\n\" stop t))"
            );
            let mut peer = accept(&listener);
            let mut bytes = [0; 7];
            peer.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"stopped");
            peer.write_all(b"held-input").unwrap();
            eval(&mut i, "(accept-process-output nil 0.03)");
            assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");
            assert_eq!(
                printed(&mut i, "(eq (continue-process nw-client) nw-client)"),
                "t"
            );
            wait_for(
                &mut i,
                "(equal (process-get nw-client 'received) \"held-input\")",
            );
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "open");
            eval(&mut i, "(delete-process nw-client)");
            assert_closed(fd);
            drop(peer);

            // Calling send without polling must wait for completion, deliver
            // the open sentinel, and put real bytes onto the TCP connection.
            let process = connect(&mut i, address, ":sentinel #'nw-sentinel");
            let fd = pending_fd(&process);
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
            assert_eq!(
                printed(&mut i, "(process-send-string nw-client \"early\")"),
                "nil"
            );
            assert_eq!(printed(&mut i, "(process-status nw-client)"), "open");
            assert_eq!(
                printed(&mut i, "(process-get nw-client 'events)"),
                "((sentinel \"open\\n\" open t))"
            );
            let mut peer = accept(&listener);
            let mut bytes = [0; 5];
            peer.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"early");
            eval(&mut i, "(delete-process nw-client)");
            assert_closed(fd);
        }
    });
}

/// Reserve the endpoint until our nonblocking connect has started. macOS
/// blackholes SYNs to a merely bound socket, so release it after connect.
/// wait_for_rejection verifies that no reused listener became a peer.
fn refused_endpoint() -> (OwnedFd, SocketAddr) {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    assert!(fd >= 0);
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut address: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        address.sin_len = std::mem::size_of::<libc::sockaddr_in>() as _;
    }
    address.sin_family = libc::AF_INET as _;
    address.sin_addr.s_addr = u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets());
    let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
    assert_eq!(
        unsafe {
            libc::bind(
                socket.as_raw_fd(),
                (&address as *const libc::sockaddr_in).cast(),
                length,
            )
        },
        0
    );
    assert_eq!(
        unsafe {
            libc::getsockname(
                socket.as_raw_fd(),
                (&mut address as *mut libc::sockaddr_in).cast(),
                &mut length,
            )
        },
        0
    );
    (
        socket,
        SocketAddr::from((Ipv4Addr::LOCALHOST, u16::from_be(address.sin_port))),
    )
}

/// Wait for kernel completion without consuming SO_ERROR: the interpreter
/// must still read and preserve ECONNREFUSED itself. getpeername can report
/// EINVAL on macOS and ENOTCONN elsewhere; neither consumes the pending error.
fn wait_for_rejection(fd: RawFd) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut descriptor = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    loop {
        let ready = unsafe { libc::poll(&mut descriptor, 1, 20) };
        if ready > 0 {
            break;
        }
        if ready < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::Interrupted
            );
        }
        assert!(
            Instant::now() < deadline,
            "owned connect did not complete in the kernel"
        );
    }
    assert_eq!(descriptor.revents & libc::POLLNVAL, 0);
    let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
    assert_eq!(
        unsafe {
            libc::getpeername(
                fd,
                (&mut address as *mut libc::sockaddr_storage).cast(),
                &mut length,
            )
        },
        -1,
        "a reused listener must not become a usable destination"
    );
}

#[test]
fn nowait_refusal_is_deferred_and_preserves_errno() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        setup(&mut i);
        let (reservation, address) = refused_endpoint();
        let process = connect(&mut i, address, ":sentinel #'nw-sentinel");
        let fd = pending_fd(&process);
        assert_nonblocking(fd);
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
        assert_eq!(printed(&mut i, "(process-get nw-client 'events)"), "nil");
        drop(reservation);
        wait_for_rejection(fd);
        wait_for(&mut i, "(eq (process-status nw-client) 'failed)");
        assert_closed(fd);
        assert_eq!(printed(&mut i, "(process-live-p nw-client)"), "nil");
        assert_eq!(
            printed(&mut i, "(process-exit-status nw-client)"),
            libc::ECONNREFUSED.to_string()
        );
        assert_eq!(
            printed(&mut i, "(process-get nw-client 'events)"),
            format!(
                "((sentinel \"failed with code {}\\n\" failed nil))",
                libc::ECONNREFUSED
            )
        );
        assert_eq!(
            printed(
                &mut i,
                "(eq (get-process (process-name nw-client)) nw-client)"
            ),
            "t"
        );
        eval(&mut i, "(accept-process-output nil 0.03)");
        assert_eq!(
            printed(
                &mut i,
                "(eq (get-process (process-name nw-client)) nw-client)"
            ),
            "t"
        );
        assert_eq!(
            printed(&mut i, "(length (process-get nw-client 'events))"),
            "1"
        );
        assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");
        // GNU retains failed network processes until explicitly deleted.
        eval(&mut i, "(delete-process nw-client)");
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "closed");
        assert_eq!(printed(&mut i, "(process-exit-status nw-client)"), "0");
        assert_eq!(
            printed(&mut i, "(get-process (process-name nw-client))"),
            "nil"
        );
        assert_eq!(
            printed(&mut i, "(process-get nw-client 'events)"),
            format!(
                "((sentinel \"failed with code {}\\n\" failed nil) (sentinel \"deleted\\n\" closed nil))",
                libc::ECONNREFUSED
            )
        );

        let (reservation, address) = refused_endpoint();
        let process = connect(&mut i, address, ":sentinel #'nw-sentinel");
        let fd = pending_fd(&process);
        drop(reservation);
        wait_for_rejection(fd);
        let name = printed(&mut i, "(process-name nw-client)");
        assert_eq!(
            printed(
                &mut i,
                "(condition-case err (process-send-string nw-client \"early\") (error err))"
            ),
            format!(
                "(error \"Process {} not running: failed with code {}\\n\")",
                name.trim_matches('"'),
                libc::ECONNREFUSED
            )
        );
        assert_closed(fd);
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "failed");
        assert_eq!(
            printed(&mut i, "(length (process-get nw-client 'events))"),
            "1"
        );
    });
}

#[cfg(target_os = "linux")]
struct SaturatedListener {
    address: SocketAddr,
    _fillers: Vec<TcpStream>,
    release: Option<std::sync::mpsc::Sender<()>>,
    guard: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl SaturatedListener {
    fn new() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        // Linux counts backlog + 1 completed connections. Fill until an owned
        // extra connection times out, proving the queue is genuinely full.
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
        let address = listener.local_addr().unwrap();
        let mut fillers = Vec::new();
        let mut saturated = false;
        for _ in 0..16 {
            match TcpStream::connect_timeout(&address, Duration::from_millis(80)) {
                Ok(stream) => fillers.push(stream),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    saturated = true;
                    break;
                }
                Err(e) => panic!("backlog probe must time out, not fail: {e}"),
            }
        }
        assert!(
            saturated && !fillers.is_empty(),
            "failed to saturate owned backlog"
        );
        let (release, receiver) = std::sync::mpsc::channel();
        let guard = std::thread::spawn(move || {
            // Bound the legacy blocking implementation too: closing our
            // listener causes its next loopback retransmit to be rejected.
            let _ = receiver.recv_timeout(Duration::from_secs(3));
            drop(listener);
        });
        Self {
            address,
            _fillers: fillers,
            release: Some(release),
            guard: Some(guard),
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for SaturatedListener {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(guard) = self.guard.take() {
            guard.join().unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn nowait_saturated_backlog_returns_promptly_pumps_timers_and_cancels() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        setup(&mut i);
        let fixture = SaturatedListener::new();
        let started = Instant::now();
        let process = connect(&mut i, fixture.address, ":sentinel #'nw-sentinel");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            ":nowait constructor waited for the saturated listen queue"
        );
        let fd = pending_fd(&process);
        assert_nonblocking(fd);
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        assert_eq!(
            unsafe { libc::poll(&mut descriptor, 1, 0) },
            0,
            "connect must still be pending in the kernel"
        );
        eval(
            &mut i,
            "(setq nw-ticked nil)
             (setq nw-timer (run-at-time 0.01 nil (lambda () (setq nw-ticked t))))",
        );
        let started = Instant::now();
        eval(&mut i, "(accept-process-output nw-client 0.08)");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "poll blocked on connect"
        );
        assert_eq!(printed(&mut i, "nw-ticked"), "t");
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
        assert_eq!(printed(&mut i, "(process-get nw-client 'events)"), "nil");
        eval(&mut i, "(cancel-timer nw-timer)");
        assert_eq!(printed(&mut i, "(delete-process nw-client)"), "nil");
        // The process object remains strongly referenced in Rust and Lisp.
        assert_closed(fd);
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "closed");
        assert_eq!(printed(&mut i, "(process-exit-status nw-client)"), "0");
        assert_eq!(
            printed(&mut i, "(get-process (process-name nw-client))"),
            "nil"
        );
        assert_eq!(
            printed(&mut i, "(process-get nw-client 'events)"),
            "((sentinel \"deleted\\n\" closed nil))"
        );
        drop(fixture);
        eval(
            &mut i,
            "(accept-process-output nil 0.08) (delete-process nw-client)",
        );
        assert_eq!(
            printed(&mut i, "(process-get nw-client 'events)"),
            "((sentinel \"deleted\\n\" closed nil))"
        );
        assert_eq!(printed(&mut i, "(process-get nw-client 'received)"), "nil");

        // A send made while the kernel connect is pending must service timers.
        // Timer cancellation also exercises deletion from a reentrant callback.
        let fixture = SaturatedListener::new();
        let process = connect(&mut i, fixture.address, ":sentinel #'nw-sentinel");
        let fd = pending_fd(&process);
        let name = printed(&mut i, "(process-name nw-client)");
        eval(
            &mut i,
            "(setq nw-timer (run-at-time 0.02 nil
                       (lambda () (delete-process nw-client))))",
        );
        let started = Instant::now();
        assert_eq!(
            printed(
                &mut i,
                "(condition-case err (process-send-string nw-client \"pending\") (error err))"
            ),
            format!(
                "(error \"Process {} not running: deleted\\n\")",
                name.trim_matches('"')
            )
        );
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "send failed to pump cancellation timer while connect was pending"
        );
        assert_closed(fd);
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "closed");
        assert_eq!(
            printed(&mut i, "(process-get nw-client 'events)"),
            "((sentinel \"deleted\\n\" closed nil))"
        );
        eval(&mut i, "(cancel-timer nw-timer)");
        drop(fixture);

        // Invoke the primitive directly so the evaluator cannot consume quit
        // before the pending-connect send loop gets a chance to observe it.
        let fixture = SaturatedListener::new();
        let process = connect(&mut i, fixture.address, ":sentinel #'nw-sentinel");
        let fd = pending_fd(&process);
        let Value::Subr(send) = eval(&mut i, "(symbol-function 'process-send-string)") else {
            panic!("process-send-string must expose its native primitive");
        };
        i.quit_flag = true;
        let started = Instant::now();
        let result = (send.func)(&mut i, vec![process.clone(), Value::string("quit")].into());
        assert!(
            matches!(result, Err(ref flow) if flow.is_quit()),
            "pending send ignored quit: {result:?}"
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(!i.quit_flag, "send must consume the pending quit");
        assert_eq!(printed(&mut i, "(process-status nw-client)"), "connect");
        assert_eq!(printed(&mut i, "(process-get nw-client 'events)"), "nil");
        assert_eq!(pending_fd(&process), fd);
        assert_nonblocking(fd);
        eval(&mut i, "(delete-process nw-client)");
        assert_closed(fd);
        drop(fixture);
    });
}
