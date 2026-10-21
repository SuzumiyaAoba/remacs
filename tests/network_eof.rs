//! Kernel evidence that process-send-eof half-closes connected streams.
mod common;
use common::*;
use remacs::lisp::process::ProcIo;
use remacs::lisp::{Interp, Value};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
