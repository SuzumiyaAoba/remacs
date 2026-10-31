use super::{GuiDialogBackend, GuiRequest};
use remacs_runtime::frontend::DialogBackend;

#[test]
fn dialog_header_crosses_the_gui_transport() {
    for header in [None, Some(false), Some(true)] {
        let (requests, incoming) = smol::channel::unbounded();
        let worker = std::thread::spawn(move || {
            let GuiRequest::Prompt {
                frame_id,
                message,
                detail,
                header: actual,
                buttons,
                reply,
            } = incoming.recv_blocking().unwrap()
            else {
                panic!("expected a prompt");
            };
            assert_eq!(frame_id, 7);
            assert_eq!(message, "Body");
            assert_eq!(detail, None);
            assert_eq!(actual, header);
            assert_eq!(buttons, ["OK"]);
            reply.send(Ok(Some(0))).unwrap();
        });
        let backend = GuiDialogBackend { requests };
        let buttons = ["OK".to_string()];
        let result = match header {
            Some(header) => backend.choose_with_header(7, "Body", None, &buttons, header),
            None => backend.choose(7, "Body", None, &buttons),
        };
        assert_eq!(result.unwrap(), Some(0));
        worker.join().unwrap();
    }
}
