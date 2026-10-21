//! Kernel evidence for network EOF, listener lifetime and contact endpoints.
mod common;
use common::*;
use remacs::lisp::process::ProcIo;
use remacs::lisp::{Interp, Value};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Descriptor checks must not race another test reusing the descriptor.
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct OwnedDirectory(PathBuf);

impl OwnedDirectory {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/tmp/remacs-eof-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("alias.sock"));
        let _ = std::fs::remove_file(self.0.join("owned.sock"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|e| panic!("{source}: {e:?}"))
}

fn printed(i: &mut Interp, source: &str) -> String {
    let value = eval(i, source);
    i.prin1_to_string(&value)
}

fn fd(process: &Value) -> RawFd {
    let Value::Process(p) = process else {
        panic!("process")
    };
    match &p.borrow().io {
        ProcIo::Net(stream) => stream.as_raw_fd(),
        ProcIo::LocalNet(stream) => stream.as_raw_fd(),
        ProcIo::Connecting(connect) => connect.as_raw_fd(),
        ProcIo::Listen(listener) => listener.as_raw_fd(),
        ProcIo::LocalListen(listener, _) => listener.as_raw_fd(),
        _ => panic!("connected or connecting stream"),
    }
}

fn accept<T>(mut attempt: impl FnMut() -> std::io::Result<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match attempt() {
            Ok(stream) => return stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "owned accept timed out");
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => panic!("owned accept: {e}"),
        }
    }
}

fn connect(i: &mut Interp, options: &str) -> Value {
    eval(
        i,
        &format!(
            r#"(setq eof217 (make-network-process :name "owned-eof"
        :coding 'binary :noquery t :sentinel #'ignore
        :filter (lambda (p text) (process-put p 'text (concat (process-get p 'text) text)))
        {options}))"#
        ),
    )
}

fn finish_write(i: &mut Interp, process: &Value) -> RawFd {
    let socket = fd(process);
    assert_eq!(printed(i, "(eq (process-send-eof eof217) eof217)"), "t");
    assert_eq!(printed(i, "(process-status eof217)"), "open");
    assert_eq!(
        printed(i, "(process-send-string eof217 \"DISCARDED\")"),
        "nil"
    );
    assert_eq!(printed(i, "(eq (process-send-eof eof217) eof217)"), "t");
    assert_eq!(fd(process), socket, "EOF must preserve the read socket");
    assert!(unsafe { libc::fcntl(socket, libc::F_GETFD) } >= 0);
    socket
}

fn check_peer(i: &mut Interp, mut peer: impl Read + Write, expected: &[u8], socket: RawFd) {
    let mut received = Vec::new();
    peer.read_to_end(&mut received)
        .expect("peer must see real kernel EOF");
    assert_eq!(received, expected, "later sends must not reach the wire");
    peer.write_all(b"REPLY").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while printed(i, "(equal (process-get eof217 'text) \"REPLY\")") != "t" {
        assert!(
            Instant::now() < deadline,
            "receive side stopped after write EOF"
        );
        eval(i, "(accept-process-output eof217 0.01 nil t)");
    }
    assert_eq!(printed(i, "(process-status eof217)"), "open");
    eval(i, "(delete-process eof217)");
    assert_eq!(unsafe { libc::fcntl(socket, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

#[test]
fn stream_eof_preserves_reads_discards_later_writes_and_closes_descriptors() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        let mut listeners = vec![TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap()];
        if let Ok(ipv6) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
            listeners.push(ipv6);
        }
        for listener in listeners {
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            for nowait in [false, true] {
                let process = connect(
                    &mut i,
                    &format!(
                        ":host {:?} :family '{} :service {} :nowait {}",
                        address.ip().to_string(),
                        if address.is_ipv4() { "ipv4" } else { "ipv6" },
                        address.port(),
                        if nowait { "t" } else { "nil" }
                    ),
                );
                if !nowait {
                    eval(&mut i, "(process-send-string eof217 \"BEFORE\")");
                }
                // In the nowait case EOF itself must finish the connect.
                let socket = finish_write(&mut i, &process);
                let peer = accept(|| listener.accept().map(|(s, _)| s));
                peer.set_nonblocking(false).unwrap();
                peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                peer.set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                check_peer(&mut i, peer, if nowait { b"" } else { b"BEFORE" }, socket);
            }
        }
        let directory = OwnedDirectory::new();
        let path = directory.0.join("owned.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let process = connect(
            &mut i,
            &format!(":family 'local :service {:?}", path.to_str().unwrap()),
        );
        eval(
            &mut i,
            "(set-process-coding-system eof217 'binary 'iso-2022-jp) (process-send-string eof217 \"あ\")",
        );
        let socket = finish_write(&mut i, &process);
        let peer = accept(|| listener.accept().map(|(s, _)| s));
        peer.set_nonblocking(false).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        check_peer(&mut i, peer, b"\x1b$B$\x22", socket);
    });
}

fn closed(socket: RawFd) {
    assert_eq!(unsafe { libc::fcntl(socket, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

fn reuse_descriptor(source: RawFd, target: RawFd) -> Option<OwnedFd> {
    if source == target {
        return None;
    }
    // TARGET was just verified closed and all tests in this process are
    // serialized. Force reuse instead of depending on allocator behavior.
    assert_eq!(unsafe { libc::dup2(source, target) }, target);
    Some(unsafe { OwnedFd::from_raw_fd(target) })
}

#[test]
fn deleted_listeners_release_ports_paths_and_recycled_descriptors() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        for host in ["127.0.0.1", "::1"] {
            if host == "::1" && TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_err() {
                continue;
            }
            let family = if host == "::1" { "ipv6" } else { "ipv4" };
            let process = eval(
                &mut i,
                &format!(
                    r#"(setq listener218 (make-network-process
                :name "owned-listener" :host {host:?} :family '{family} :server t :service 0
                :noquery t :sentinel #'ignore :filter #'ignore))"#
                ),
            );
            let socket = fd(&process);
            let address = {
                let Value::Process(p) = &process else {
                    unreachable!()
                };
                let p = p.borrow();
                let ProcIo::Listen(listener) = &p.io else {
                    unreachable!()
                };
                listener.local_addr().unwrap()
            };
            eval(&mut i, "(delete-process listener218)");
            closed(socket);
            let replacement =
                TcpListener::bind(address).expect("held deleted process must release its port");
            replacement.set_nonblocking(true).unwrap();
            let _reused = reuse_descriptor(replacement.as_raw_fd(), socket);
            eval(
                &mut i,
                "(delete-process listener218) (delete-process listener218)",
            );
            assert!(unsafe { libc::fcntl(socket, libc::F_GETFD) } >= 0);
            let mut client =
                std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
            client.write_all(b"PING").unwrap();
            let mut peer = accept(|| replacement.accept().map(|(stream, _)| stream));
            peer.set_nonblocking(false).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut bytes = [0; 4];
            peer.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"PING");
        }
        let directory = OwnedDirectory::new();
        let path = directory.0.join("owned.sock");
        let process = eval(
            &mut i,
            &format!(
                r#"(setq listener218 (make-network-process
            :name "owned-local-listener" :family 'local :server t :service {:?}
            :noquery t :sentinel #'ignore :filter #'ignore))"#,
                path.to_str().unwrap()
            ),
        );
        let socket = fd(&process);
        assert!(path.exists());
        eval(&mut i, "(delete-process listener218)");
        closed(socket);
        assert!(
            path.exists(),
            "GNU preserves the local socket pathname on delete"
        );
        std::fs::remove_file(&path).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        replacement.set_nonblocking(true).unwrap();
        let _reused = reuse_descriptor(replacement.as_raw_fd(), socket);
        eval(
            &mut i,
            "(delete-process listener218) (delete-process listener218)",
        );
        assert!(
            path.exists(),
            "deleting the old object again must preserve the replacement path"
        );
        assert!(unsafe { libc::fcntl(socket, libc::F_GETFD) } >= 0);
        let mut client = std::os::unix::net::UnixStream::connect(&path).unwrap();
        client.write_all(b"PING").unwrap();
        let mut peer = accept(|| replacement.accept().map(|(stream, _)| stream));
        peer.set_nonblocking(false).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"PING");
    });
}

fn contact_address(i: &mut Interp, expression: &str) -> std::net::SocketAddr {
    let value = eval(i, &format!("(format-network-address {expression})"));
    let Value::Str(text) = value else {
        panic!("missing socket address: {expression}")
    };
    let address = text
        .borrow()
        .parse()
        .unwrap_or_else(|e| panic!("{expression}: {e}"));
    address
}

fn contact_bytes(i: &mut Interp, expression: &str) -> Vec<u8> {
    let value = eval(i, expression);
    let Value::Str(text) = value else {
        panic!("missing local address: {expression}")
    };
    assert!(
        i.is_unibyte_str(&text),
        "Unix addresses must preserve raw bytes"
    );
    let bytes = text
        .borrow()
        .chars()
        .map(|c| u8::try_from(c as u32).unwrap())
        .collect();
    bytes
}

fn anonymous_address(socket: RawFd) -> Vec<u8> {
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    // Detect padding that was not actually returned by the kernel rather
    // than accidentally agreeing with the implementation's zeroed buffer.
    unsafe {
        std::ptr::write_bytes(
            (&mut address as *mut libc::sockaddr_un).cast::<u8>(),
            0xa5,
            std::mem::size_of_val(&address),
        );
    }
    let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
    assert_eq!(
        unsafe {
            libc::getsockname(
                socket,
                (&mut address as *mut libc::sockaddr_un).cast(),
                &mut length,
            )
        },
        0
    );
    let header = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    let count = (length as usize)
        .min(std::mem::size_of_val(&address))
        .saturating_sub(header);
    unsafe { std::slice::from_raw_parts(address.sun_path.as_ptr().cast(), count).to_vec() }
}

fn wait_accepted(i: &mut Interp, phase: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !eval(i, "(processp contact220-peer)").truthy() {
        assert!(
            Instant::now() < deadline,
            "{phase}: contact test accept timed out"
        );
        eval(i, "(accept-process-output nil 0.01)");
    }
}

#[test]
fn contact_endpoints_match_kernel_addresses_and_resolved_address_families() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = v4.local_addr().unwrap().port();
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).ok();
        v4.set_nonblocking(true).unwrap();
        if let Some(listener) = &v6 {
            listener.set_nonblocking(true).unwrap();
        }
        for (family, host) in [
            ("ipv4", "127.0.0.1"),
            ("ipv4", "localhost"),
            ("ipv6", "::1"),
            ("ipv6", "localhost"),
        ] {
            if family == "ipv6" && v6.is_none() {
                continue;
            }
            // Every candidate is an owned loopback endpoint. In particular,
            // the other family also listens on this port, so ignoring the
            // requested family cannot hide behind connection refusal.
            use std::net::ToSocketAddrs;
            let candidates: Vec<_> = (host, port).to_socket_addrs().unwrap().collect();
            assert!(candidates.iter().all(|a| a.ip().is_loopback()));
            if !candidates.iter().any(|a| a.is_ipv4() == (family == "ipv4")) {
                continue;
            }
            let listener = if family == "ipv4" {
                &v4
            } else {
                v6.as_ref().unwrap()
            };
            for nowait in [false, true] {
                let process = connect(
                    &mut i,
                    &format!(
                        ":host {host:?} :family '{family} :service {port} :nowait {}",
                        if nowait { "t" } else { "nil" }
                    ),
                );
                eval(&mut i, "(process-send-string eof217 \"READY\")");
                let mut peer = accept(|| listener.accept().map(|(stream, _)| stream));
                peer.set_nonblocking(false).unwrap();
                peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut data = [0; 5];
                peer.read_exact(&mut data).unwrap();
                assert_eq!(&data, b"READY");
                assert_eq!(
                    contact_address(&mut i, "(process-contact eof217 :remote)"),
                    peer.local_addr().unwrap()
                );
                assert_eq!(
                    contact_address(&mut i, "(process-contact eof217 :local)"),
                    peer.peer_addr().unwrap()
                );
                assert_eq!(
                    printed(&mut i, "(process-contact eof217 :host)"),
                    format!("{host:?}")
                );
                let socket = fd(&process);
                eval(&mut i, "(delete-process eof217)");
                closed(socket);
            }
        }
        drop(v6);
        drop(v4);
        eval(&mut i, "(defvar contact220-peer nil) (defun contact220-log (_server peer _message)
            (process-put peer 'at-log-local (process-contact peer :local))
            (process-put peer 'at-log-remote (process-contact peer :remote)) (setq contact220-peer peer))");
        for host in ["127.0.0.1", "::1"] {
            if host == "::1" && TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_err() {
                continue;
            }
            let family = if host == "::1" { "ipv6" } else { "ipv4" };
            let server = eval(
                &mut i,
                &format!(
                    "(setq contact220-peer nil contact220-server (make-network-process :name \"contact-server\" :host {host:?} :family '{family} :service 0 :server t :noquery t :filter #'ignore :sentinel #'ignore :log #'contact220-log))"
                ),
            );
            let address = {
                let Value::Process(p) = &server else {
                    unreachable!()
                };
                let p = p.borrow();
                let ProcIo::Listen(l) = &p.io else {
                    unreachable!()
                };
                l.local_addr().unwrap()
            };
            assert_eq!(
                contact_address(&mut i, "(process-contact contact220-server :local)"),
                address
            );
            let client =
                std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
            wait_accepted(&mut i, "INET");
            for expression in [
                "(process-contact contact220-peer :remote)",
                "(process-get contact220-peer 'at-log-remote)",
            ] {
                assert_eq!(
                    contact_address(&mut i, expression),
                    client.local_addr().unwrap()
                );
            }
            for expression in [
                "(process-contact contact220-peer :local)",
                "(process-get contact220-peer 'at-log-local)",
            ] {
                assert_eq!(
                    contact_address(&mut i, expression),
                    client.peer_addr().unwrap()
                );
            }
            assert_eq!(
                printed(&mut i, "(process-contact contact220-peer :server)"),
                "nil"
            );
            assert_eq!(
                printed(&mut i, "(process-contact contact220-server :server)"),
                "t"
            );
            eval(
                &mut i,
                "(delete-process contact220-peer) (delete-process contact220-server)",
            );
            assert_eq!(
                contact_address(&mut i, "(process-contact contact220-peer :remote)"),
                client.local_addr().unwrap()
            );
        }
        let directory = OwnedDirectory::new();
        let path = directory.0.join("owned.sock");
        eval(
            &mut i,
            &format!(
                "(setq contact220-peer nil contact220-server (make-network-process :name \"contact-local\" :family 'local :service {:?} :server t :noquery t :filter #'ignore :sentinel #'ignore :log #'contact220-log))",
                path.to_str().unwrap()
            ),
        );
        let client = std::os::unix::net::UnixStream::connect(&path).unwrap();
        wait_accepted(&mut i, "Unix connected");
        let accepted = eval(&mut i, "contact220-peer");
        let flags = unsafe { libc::fcntl(fd(&accepted), libc::F_GETFD) };
        assert!(flags >= 0 && flags & libc::FD_CLOEXEC != 0);
        assert_eq!(
            contact_bytes(&mut i, "(process-contact contact220-peer :remote)"),
            anonymous_address(client.as_raw_fd())
        );
        assert_eq!(
            contact_bytes(&mut i, "(process-contact contact220-peer :local)"),
            path.to_str().unwrap().as_bytes()
        );
        eval(
            &mut i,
            "(delete-process contact220-peer) (setq contact220-peer nil)",
        );
        drop(client);
        let short_client = std::os::unix::net::UnixStream::connect(&path).unwrap();
        let short_address = anonymous_address(short_client.as_raw_fd());
        drop(short_client);
        wait_accepted(&mut i, "Unix early close");
        assert_eq!(
            contact_bytes(&mut i, "(process-contact contact220-peer :remote)"),
            short_address
        );
        eval(
            &mut i,
            "(delete-process contact220-peer) (delete-process contact220-server)",
        );
        std::fs::remove_file(&path).unwrap();
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let alias = directory.0.join("alias.sock");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let process = connect(
            &mut i,
            &format!(":family 'local :service {:?}", alias.to_str().unwrap()),
        );
        let _peer = accept(|| listener.accept().map(|(stream, _)| stream));
        assert_eq!(
            contact_bytes(&mut i, "(process-contact eof217 :local)"),
            anonymous_address(fd(&process))
        );
        assert_eq!(
            contact_bytes(&mut i, "(process-contact eof217 :remote)"),
            alias.to_str().unwrap().as_bytes()
        );
        eval(&mut i, "(delete-process eof217)");
    });
}

fn check_paused_stream(i: &mut Interp, process: &Value, mut peer: impl Read + Write) {
    eval(i, "(stop-process eof217) (stop-process eof217)");
    assert_eq!(printed(i, "(process-status eof217)"), "stop");
    assert_eq!(printed(i, "(process-command eof217)"), "t");
    peer.write_all(b"DATA").unwrap();
    eval(i, "(accept-process-output nil 0.03)");
    assert_eq!(printed(i, "(process-get eof217 'text)"), "nil");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut pending = [0; 4];
    loop {
        let size = unsafe {
            libc::recv(
                fd(process),
                pending.as_mut_ptr().cast(),
                pending.len(),
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if size == 4 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "paused input was consumed before resume"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(&pending, b"DATA");
    eval(i, "(process-send-string eof217 \"OUT\")");
    let mut output = [0; 3];
    peer.read_exact(&mut output).unwrap();
    assert_eq!(&output, b"OUT", "stopping input must preserve writes");
    eval(i, "(continue-process eof217)");
    assert_eq!(printed(i, "(process-status eof217)"), "open");
    let deadline = Instant::now() + Duration::from_secs(3);
    while printed(i, "(process-get eof217 'text)") != "\"DATA\"" {
        assert!(
            Instant::now() < deadline,
            "resume did not deliver queued input"
        );
        eval(i, "(accept-process-output eof217 0.01 nil t)");
    }
    assert_eq!(printed(i, "(process-command eof217)"), "nil");
    eval(i, "(delete-process eof217)");
}

#[test]
fn network_stop_preserves_kernel_input_until_resume_and_keeps_writes_live() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        let mut listeners = vec![TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap()];
        if let Ok(v6) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
            listeners.push(v6);
        }
        for listener in listeners {
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let process = connect(
                &mut i,
                &format!(
                    ":host {:?} :family '{} :service {}",
                    address.ip().to_string(),
                    if address.is_ipv4() { "ipv4" } else { "ipv6" },
                    address.port()
                ),
            );
            let peer = accept(|| listener.accept().map(|(stream, _)| stream));
            peer.set_nonblocking(false).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            peer.set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            check_paused_stream(&mut i, &process, peer);
        }
        let directory = OwnedDirectory::new();
        let path = directory.0.join("owned.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let process = connect(
            &mut i,
            &format!(":family 'local :service {:?}", path.to_str().unwrap()),
        );
        let peer = accept(|| listener.accept().map(|(stream, _)| stream));
        peer.set_nonblocking(false).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        check_paused_stream(&mut i, &process, peer);
    });
}
