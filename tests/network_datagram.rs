//! Kernel-level evidence for #177, independent of process-contact metadata.
//! Use only self-owned loopback sockets. Never contact an external service.
mod common;
use common::*;
use remacs::lisp::process::{DatagramSocket, ProcIo};
use remacs::lisp::{Interp, Value};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, UdpSocket};
use std::time::Duration;

fn eval(i: &mut Interp, code: &str) -> Value {
    i.eval_str(code).unwrap_or_else(|e| panic!("{code}: {e:?}"))
}
fn printed(i: &mut Interp, code: &str) -> String {
    let value = eval(i, code);
    i.prin1_to_string(&value).to_unicode_lossy()
}
fn address(process: &Value) -> SocketAddr {
    let Value::Process(p) = process else {
        panic!("expected process")
    };
    let p = p.borrow();
    let ProcIo::Datagram(io) = &p.io else {
        panic!("must be datagram backend")
    };
    let DatagramSocket::Inet(socket) = &io.socket else {
        panic!("must be UDP")
    };
    assert!(
        socket.peer_addr().is_err(),
        "GNU datagrams are never connect()ed"
    );
    socket.local_addr().unwrap()
}

#[test]
fn datagram_kernel_transport_boundaries_bytes_and_lifetime() {
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        eval(&mut i, "(setq text-quoting-style 'grave print-escape-newlines t)");
        let tcp = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let server = eval(
            &mut i,
            &format!(
                "(setq dg-server (make-network-process :name \"owned-udp\" \
             :type 'datagram :family 'ipv4 :host \"127.0.0.1\" :server t :service {} \
             :coding 'binary :noquery t :sentinel 'ignore \
             :filter (lambda (p text) (process-put p 'packets \
                 (append (process-get p 'packets) (list text))))))",
                tcp.local_addr().unwrap().port()
            ),
        );
        let local = address(&server);
        assert_eq!(local.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(local.port(), 0);
        assert_eq!(
            printed(
                &mut i,
                "(list (process-type dg-server) (process-status dg-server))"
            ),
            "(network open)"
        );
        // We own the TCP port already: the UDP bind proves the transport,
        // without racing some unrelated TCP listener for a random port.
        assert_eq!(tcp.local_addr().unwrap(), local);
        let peer = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        peer.send_to(b"first", local).unwrap();
        peer.send_to(b"second", local).unwrap();
        assert_eq!(printed(&mut i, "(accept-process-output dg-server 1)"), "t");
        assert_eq!(
            printed(&mut i, "(process-get dg-server 'packets)"),
            "(\"first\" \"second\")"
        );
        eval(&mut i, "(process-send-string dg-server \"reply\")");
        let mut bytes = vec![0; 65536];
        let (n, sender) = peer.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..n], b"reply");
        assert_eq!(sender, local);

        // Arbitrary octets survive both directions, including NUL and invalid
        // UTF-8. These are real bytes, not just matching lossy Lisp strings.
        let raw = [0, 1, 127, 128, 192, 254, 255];
        peer.send_to(&raw, local).unwrap();
        eval(&mut i, "(accept-process-output dg-server 1)");
        assert_eq!(
            printed(
                &mut i,
                "(string-to-list (car (last (process-get dg-server 'packets))))"
            ),
            "(0 1 127 128 192 254 255)"
        );
        assert_eq!(
            printed(
                &mut i,
                "(multibyte-string-p (car (last (process-get dg-server 'packets))))"
            ),
            "nil"
        );
        eval(
            &mut i,
            "(process-send-string dg-server (unibyte-string 0 1 127 128 192 254 255))",
        );
        let (n, _) = peer.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..n], raw);

        #[cfg(target_os = "linux")]
        for size in [9000usize, 60000, 65507] {
            let raw: Vec<u8> = (0..size).map(|n| (n % 256) as u8).collect();
            eval(&mut i, "(process-put dg-server 'packets nil)");
            peer.send_to(&raw, local).unwrap();
            eval(&mut i, "(accept-process-output dg-server 1)");
            assert_eq!(
                printed(&mut i, "(length (process-get dg-server 'packets))"),
                "1"
            );
            assert_eq!(
                printed(&mut i, "(length (car (process-get dg-server 'packets)))"),
                size.to_string()
            );
            eval(
                &mut i,
                "(process-send-string dg-server (car (process-get dg-server 'packets)))",
            );
            let (n, _) = peer.recv_from(&mut bytes).unwrap();
            assert_eq!(&bytes[..n], raw);
        }

        // Nonblocking clients have no preallocated port and accept packets
        // from any sender; the Lisp suite checks per-callback reply addresses.
        let client = eval(
            &mut i,
            &format!(
                "(setq dg-client (make-network-process \
            :name \"owned-client\" :type 'datagram :host \"127.0.0.1\" :family 'ipv4 \
            :service {} :nowait t :noquery t :sentinel 'ignore))",
                local.port()
            ),
        );
        assert_eq!(
            address(&client),
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
        );
        eval(&mut i, "(process-send-string dg-client \"from-client\")");
        assert_ne!(address(&client).port(), 0);
        eval(&mut i, "(accept-process-output dg-server 1)");
        eval(&mut i, "(delete-process dg-client)");

        // Bind failures cannot register a process or leak its reserved name.
        let count = i.processes.len();
        let failure = format!(
            "(condition-case err (make-network-process \
             :name \"owned-conflict\" :type 'datagram :family 'ipv4 :host \"127.0.0.1\" \
             :server t :service {}) (error (list (car err) (cadr err))))",
            local.port()
        );
        assert_eq!(
            printed(&mut i, &failure),
            "(file-error \"Cannot bind server socket\")"
        );
        assert_eq!(i.processes.len(), count);

        // Empty Lisp send is a no-op. A real empty packet closes GNU 31.1's
        // network process; it must not become a fictitious filter callback.
        eval(&mut i, "(process-put dg-server 'packets nil)");
        eval(
            &mut i,
            "(set-process-sentinel dg-server (lambda (p event) (process-put p 'event event)))",
        );
        peer.send_to(b"", local).unwrap();
        eval(&mut i, "(accept-process-output dg-server 0.1)");
        assert_eq!(
            printed(
                &mut i,
                "(list (process-status dg-server) (process-exit-status dg-server) (process-get dg-server 'packets))"
            ),
            "(closed 256 nil)"
        );
        assert_eq!(
            printed(&mut i, "(process-get dg-server 'event)"),
            "\"connection broken by remote peer\\n\""
        );
        assert!(
            UdpSocket::bind(local).is_ok(),
            "empty-receive closes socket immediately"
        );
        eval(&mut i, "(delete-process dg-server)");
        drop(tcp);

        // Deletion also closes before the Lisp/host references are dropped.
        let server = eval(
            &mut i,
            "(setq dg-delete (make-network-process :name \"owned-delete\" \
            :type 'datagram :host \"127.0.0.1\" :server t :service 0 :noquery t :sentinel 'ignore))",
        );
        let local = address(&server);
        eval(&mut i, "(delete-process dg-delete)");
        let replacement = UdpSocket::bind(local).unwrap();
        eval(&mut i, "(delete-process dg-delete)");
        peer.send_to(b"still-open", replacement.local_addr().unwrap())
            .unwrap();
        replacement
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!(replacement.recv_from(&mut bytes).unwrap().0, 10);
        drop(server);

        // Empty reads first flush pending decoder bytes; only a subsequent
        // empty packet closes. GNU represents the incomplete E2 as byte8.
        let utf8 = eval(
            &mut i,
            "(setq dg-utf8 (make-network-process :name \"owned-utf8\" \
            :type 'datagram :host \"127.0.0.1\" :server t :service 0 :coding 'utf-8 \
            :noquery t :sentinel 'ignore :filter (lambda (p s) (process-put p 'data (string-to-list s)))))",
        );
        peer.send_to(&[226], address(&utf8)).unwrap();
        eval(&mut i, "(accept-process-output dg-utf8 1)");
        assert_eq!(printed(&mut i, "(process-get dg-utf8 'data)"), "nil");
        peer.send_to(b"", address(&utf8)).unwrap();
        eval(&mut i, "(accept-process-output dg-utf8 1)");
        assert_eq!(printed(&mut i, "(process-get dg-utf8 'data)"), "(4194274)");
        assert_eq!(printed(&mut i, "(process-status dg-utf8)"), "open");
        peer.send_to(b"", address(&utf8)).unwrap();
        eval(&mut i, "(accept-process-output dg-utf8 0.1)");
        assert_eq!(printed(&mut i, "(process-status dg-utf8)"), "closed");
        eval(&mut i, "(delete-process dg-utf8)");

        // IPv6 capability probing and receivers stay on ::1.
        if let Ok(v6peer) = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0)) {
            let v6 = eval(
                &mut i,
                "(setq dg-v6 (make-network-process :name \"owned-v6\" \
                :type 'datagram :family 'ipv6 :host \"::1\" :server t :service 0 \
                :noquery t :sentinel 'ignore :filter (lambda (p s) (process-send-string p s))))",
            );
            assert_eq!(address(&v6).ip(), Ipv6Addr::LOCALHOST);
            v6peer.send_to(b"ipv6", address(&v6)).unwrap();
            eval(&mut i, "(accept-process-output dg-v6 1)");
            v6peer
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let (n, addr) = v6peer.recv_from(&mut bytes).unwrap();
            assert_eq!(&bytes[..n], b"ipv6");
            assert_eq!(addr, address(&v6));
            eval(&mut i, "(delete-process dg-v6)");
        }
    });
}
