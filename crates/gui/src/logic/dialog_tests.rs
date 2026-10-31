use super::{GuiDialogBackend, GuiRequest};
use remacs_runtime::frontend::{DialogBackend, DialogItem};

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
                items,
                reply,
            } = incoming.recv_blocking().unwrap()
            else {
                panic!("expected a prompt");
            };
            assert_eq!(frame_id, 7);
            assert_eq!(message, "Body");
            assert_eq!(detail, None);
            assert_eq!(actual, header);
            assert_eq!(items, [DialogItem::Button("OK".into())]);
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

#[test]
fn dialog_text_items_keep_order_and_only_buttons_have_return_indices() {
    let items = vec![
        DialogItem::Text("Before".into()),
        DialogItem::Button("One".into()),
        DialogItem::Text("Between".into()),
        DialogItem::Button("Two".into()),
        DialogItem::Text("After".into()),
    ];
    for (reply_index, expected) in [(Some(3), Some(1)), (Some(2), None), (None, None)] {
        let (requests, incoming) = smol::channel::unbounded();
        let original = items.clone();
        let worker = std::thread::spawn(move || {
            let GuiRequest::Prompt {
                items,
                header,
                reply,
                ..
            } = incoming.recv_blocking().unwrap()
            else {
                panic!("expected a prompt");
            };
            assert_eq!(items, original);
            assert_eq!(header, Some(false));
            reply.send(Ok(reply_index)).unwrap();
        });
        let backend = GuiDialogBackend { requests };
        assert_eq!(
            backend
                .choose_with_items(7, "Body", None, &items, false)
                .unwrap(),
            expected
        );
        worker.join().unwrap();
    }
}
