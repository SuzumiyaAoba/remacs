//! Check the kernel's bound address, not just `process-contact`: its
//! cached :local value can conceal an accidentally wildcard listener.
mod common;

use common::*;
use remacs::lisp::process::ProcIo;
use remacs::lisp::{Interp, Value};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn server_address(i: &mut Interp, source: &str) -> SocketAddr {
    let Value::Process(process) = eval(i, source) else {
        panic!("make-network-process did not return a process");
    };
    let process = process.borrow();
    let ProcIo::Listen(listener) = &process.io else {
        panic!("server did not create a TCP listener");
    };
    listener
        .local_addr()
        .expect("query the actual bound address")
}

#[test]
fn network_server_preserves_loopback_bind_address() {
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = true;
        eval(&mut i, "(setq text-quoting-style 'grave)");

        // Explicit family must never broaden an explicit loopback address.
        // Also cover omitted/nil family, and nil/omitted host defaults.
        for options in [
            ":host \"127.0.0.1\" :family 'ipv4",
            ":host \"127.0.0.1\"",
            ":host \"127.0.0.1\" :family nil",
            ":host nil :family 'ipv4",
            ":family 'ipv4",
        ] {
            for service in ["0", "t", "\"0\""] {
                let source = format!(
                    "(setq qa-bind-server (make-network-process :name \"qa-bind\" \
                 :server t :service {service} :noquery t :sentinel 'ignore {options}))"
                );
                let addr = server_address(&mut i, &source);
                assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST), "{options}");
                assert_ne!(addr.port(), 0);
                let status = eval(&mut i, "(process-status qa-bind-server)");
                assert_eq!(i.prin1_to_string(&status), "listen");
                let port = eval(&mut i, "(process-contact qa-bind-server :service)");
                assert_eq!(i.prin1_to_string(&port), addr.port().to_string());

                // The listener still accepts an ordinary loopback client.
                eval(
                    &mut i,
                    "(setq qa-bind-client (make-network-process :name \"qa-bind-client\" \
                 :host \"127.0.0.1\" :family 'ipv4 \
                 :service (process-contact qa-bind-server :service) \
                 :noquery t :sentinel 'ignore))",
                );
                let Value::Process(client) = eval(&mut i, "qa-bind-client") else {
                    panic!("client did not return a process");
                };
                {
                    let client = client.borrow();
                    let ProcIo::Net(stream) = &client.io else {
                        panic!("client did not create a TCP stream");
                    };
                    assert_eq!(stream.peer_addr().unwrap(), addr);
                }
                eval(&mut i, "(delete-process qa-bind-client)");
                eval(&mut i, "(delete-process qa-bind-server)");
                // Drop the fixture's references too: releasing sockets while
                // deleted process objects remain reachable is a separate issue.
                eval(&mut i, "(setq qa-bind-client nil qa-bind-server nil)");
                drop(client);
                assert!(
                    TcpListener::bind(addr).is_ok(),
                    "fixture left its port bound"
                );
            }
        }

        // Preserve an explicit IPv6 loopback too, where the OS supports it.
        // The capability probe itself is bound only to loopback.
        if let Ok(probe) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
            drop(probe);
            for service in ["0", "t", "\"0\""] {
                let addr = server_address(
                    &mut i,
                    &format!(
                        "(setq qa-bind-server (make-network-process :name \"qa-bind-v6\" \
                 :host \"::1\" :family 'ipv6 :server t :service {service} :noquery t))"
                    ),
                );
                assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
                assert_ne!(addr.port(), 0);
                let port = eval(&mut i, "(process-contact qa-bind-server :service)");
                assert_eq!(i.prin1_to_string(&port), addr.port().to_string());
                eval(&mut i, "(delete-process qa-bind-server)");
                eval(&mut i, "(setq qa-bind-server nil)");
            }
        }

        // Bind errors still signal file-error and do not register a process.
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let count = i.processes.len();
        let source = format!(
            "(condition-case err \
                 (make-network-process :name \"qa-bind-conflict\" :host \"127.0.0.1\" \
                  :family 'ipv4 :server t :service {} :noquery t) \
               (error (list (car err) (cadr err))))",
            occupied.local_addr().unwrap().port()
        );
        let result = eval(&mut i, &source);
        assert_eq!(
            i.prin1_to_string(&result),
            "(file-error \"Cannot bind server socket\")"
        );
        assert_eq!(i.processes.len(), count);
        for (options, expected) in [
            (
                ":service 0 :nowait t",
                "(error \"`:server' is incompatible with `:nowait'\")",
            ),
            (":service nil", "(wrong-type-argument stringp nil)"),
        ] {
            let source = format!(
                "(condition-case err \
                     (make-network-process :name \"qa-bind-error\" :host \"127.0.0.1\" \
                      :family 'ipv4 :server t {options}) \
                   (error err))"
            );
            let result = eval(&mut i, &source);
            assert_eq!(i.prin1_to_string(&result), expected);
            assert_eq!(i.processes.len(), count);
        }
    });
}
