//! Kernel evidence for stream EOF and immediate listener deletion.
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
