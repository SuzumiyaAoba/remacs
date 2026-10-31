use super::{ClipboardRequest, ClipboardTarget, GuiClipboardBackend};
use remacs_runtime::frontend::ClipboardBackend;
use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
};

#[test]
fn selection_requests_keep_primary_separate_from_clipboard() {
    let (requests, received) = smol::channel::unbounded();
    let backend = GuiClipboardBackend {
        requests,
        ready: Arc::new(AtomicBool::new(true)),
    };
    let service = thread::spawn(move || {
        for target in [ClipboardTarget::Clipboard, ClipboardTarget::Primary] {
            let ClipboardRequest::Write(actual, text, Some(reply)) =
                received.recv_blocking().unwrap()
            else {
                panic!("write request")
            };
            assert_eq!(actual, target);
            assert_eq!(text, format!("{target:?} text"));
            reply.send(Ok(())).unwrap();
            let ClipboardRequest::Read(actual, reply) = received.recv_blocking().unwrap() else {
                panic!("read request")
            };
            assert_eq!(actual, target);
            reply
                .send(Ok(Some(format!("external {target:?}"))))
                .unwrap();
        }
    });
    backend.write_text("Clipboard text".into()).unwrap();
    assert_eq!(
        backend.read_text().unwrap().as_deref(),
        Some("external Clipboard")
    );
    backend.write_primary_text("Primary text".into()).unwrap();
    assert_eq!(
        backend.read_primary_text().unwrap().as_deref(),
        Some("external Primary")
    );
    assert_eq!(
        backend.supports_primary_selection(),
        cfg!(any(target_os = "linux", target_os = "freebsd"))
    );
    service.join().unwrap();
}

#[test]
fn selection_writes_before_ui_startup_keep_the_target_without_waiting() {
    let (requests, received) = smol::channel::unbounded();
    let backend = GuiClipboardBackend {
        requests,
        ready: Arc::new(AtomicBool::new(false)),
    };
    assert_eq!(backend.read_text().unwrap(), None);
    assert_eq!(backend.read_primary_text().unwrap(), None);
    assert!(received.try_recv().is_err());
    backend.write_primary_text("queued primary".into()).unwrap();
    let ClipboardRequest::Write(target, text, reply) = received.try_recv().unwrap() else {
        panic!("write request")
    };
    assert_eq!(target, ClipboardTarget::Primary);
    assert_eq!(text, "queued primary");
    assert!(reply.is_none());
}

#[test]
fn primary_selection_service_failures_reach_the_caller() {
    let (requests, received) = smol::channel::unbounded();
    let backend = GuiClipboardBackend {
        requests,
        ready: Arc::new(AtomicBool::new(true)),
    };
    let service = thread::spawn(move || {
        let ClipboardRequest::Read(ClipboardTarget::Primary, reply) =
            received.recv_blocking().unwrap()
        else {
            panic!("primary read")
        };
        reply.send(Err("read rejected".into())).unwrap();
        let ClipboardRequest::Write(ClipboardTarget::Primary, _, Some(reply)) =
            received.recv_blocking().unwrap()
        else {
            panic!("primary write")
        };
        reply.send(Err("write rejected".into())).unwrap();
    });
    assert_eq!(backend.read_primary_text(), Err("read rejected".into()));
    assert_eq!(
        backend.write_primary_text("text".into()),
        Err("write rejected".into())
    );
    service.join().unwrap();
    assert!(backend.read_primary_text().is_err());
    assert!(backend.write_primary_text("text".into()).is_err());
}
