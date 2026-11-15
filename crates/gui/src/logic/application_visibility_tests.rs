use super::*;
use remacs_runtime::frontend::ApplicationVisibilityBackend;

fn check_activation_request(frame_id: Option<usize>, bring_frame_to_front: bool) {
    let (requests, received) = smol::channel::unbounded();
    let backend = GuiApplicationVisibilityBackend { requests };
    let worker =
        std::thread::spawn(move || backend.activate_unhidden(frame_id, bring_frame_to_front));

    let GuiRequest::ActivateUnhideApp {
        frame_id: requested_frame,
        bring_frame_to_front: requested_front,
        reply,
    } = received.recv_blocking().unwrap()
    else {
        panic!("expected an unhide and activate request");
    };
    assert_eq!(requested_frame, frame_id);
    assert_eq!(requested_front, bring_frame_to_front);
    reply.send(Ok(())).unwrap();
    assert_eq!(worker.join().unwrap(), Ok(()));
}

fn check_unhide_request() {
    let (requests, received) = smol::channel::unbounded();
    let backend = GuiApplicationVisibilityBackend { requests };
    let worker = std::thread::spawn(move || backend.unhide());

    let GuiRequest::UnhideApp { reply } = received.recv_blocking().unwrap() else {
        panic!("expected an unhide request");
    };
    reply.send(Ok(())).unwrap();
    assert_eq!(worker.join().unwrap(), Ok(()));
}

#[test]
fn activation_request_preserves_frame_and_front_mode() {
    check_activation_request(Some(41), false);
    check_activation_request(Some(42), true);
    check_activation_request(None, true);
}

#[test]
fn unhide_request_does_not_select_an_activation_mode() {
    check_unhide_request();
}
