use super::*;
use std::{collections::HashSet, thread};

#[test]
fn close_requested_before_frame_id_is_replayed_when_bound() {
    let (sender, received) = mpsc::channel();
    let frame_events = FrameEventSender::new(sender, None, None);

    frame_events.send_delete_request_or_defer().unwrap();
    frame_events.send_delete_request_or_defer().unwrap();
    assert!(matches!(received.try_recv(), Err(mpsc::TryRecvError::Empty)));

    frame_events.set_frame_id(2276);
    assert!(matches!(
        received.try_recv(),
        Ok(GuiEvent::FrameDeleteRequest { frame_id: 2276 })
    ));
    assert!(matches!(received.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[derive(Clone, Copy)]
enum Outcome {
    Success,
    MissingNativeFrame,
    RejectReady,
    DisconnectReady,
    RejectWarp,
}

#[derive(Default, Debug)]
struct Trace {
    frames: Vec<(usize, String, FrameVisibility)>,
    snapshots_after_ready: usize,
    ready: Vec<(usize, bool)>,
    prompts: Vec<(usize, bool)>,
    palettes: Vec<(usize, bool)>,
    warps: Vec<(usize, i128, i128, bool)>,
}

fn attach(
    interp: &mut Interp,
    outcome: Outcome,
) -> (
    Rc<RefCell<ChanIo>>,
    smol::channel::Receiver<Grid>,
    thread::JoinHandle<Trace>,
) {
    let (_events, received_events) = mpsc::channel();
    let (grids, received_grids) = smol::channel::bounded(1);
    let (requests, received_requests) = smol::channel::unbounded();
    let frontend = Rc::new(RefCell::new(ChanIo::new_for_test(received_events, grids)));
    frontend.borrow_mut().gui_requests = Some(requests.clone());
    // Queue an existing frame, so readiness distinguishes it from a new one.
    frontend.borrow_mut().render(interp).unwrap();
    interp.set_frame_window_backend(Some(Rc::new(GuiFrameVisibilityBackend::new(
        requests.clone(),
        &frontend,
    ))));
    interp.set_dialog_backend(Some(Rc::new(GuiDialogBackend {
        requests: requests.clone(),
        quit_request: std::sync::Arc::new(QuitRequest::default()),
    })));
    interp.set_color_panel_backend(Some(Rc::new(GuiColorPanelBackend {
        requests: requests.clone(),
    })));
    let primary = interp.selected_frame.as_ref().unwrap().borrow().id;
    let worker = thread::spawn(move || {
        let mut trace = Trace::default();
        let mut native_frames = HashSet::new();
        while let Ok(request) = received_requests.recv_blocking() {
            match request {
                GuiRequest::FrameSnapshots(mailbox) => {
                    if let Some(snapshots) = mailbox.take_latest() {
                        trace.snapshots_after_ready +=
                            usize::from(trace.ready.iter().any(|(_, ready)| *ready));
                        native_frames.clear();
                        for snapshot in snapshots {
                            if !matches!(outcome, Outcome::MissingNativeFrame)
                                || snapshot.frame_id == primary
                            {
                                native_frames.insert(snapshot.frame_id);
                            }
                            trace.frames.push((
                                snapshot.frame_id,
                                snapshot.name,
                                snapshot.visibility,
                            ));
                        }
                    }
                }
                GuiRequest::FrameReady { frame_id, reply } => {
                    let ready = native_frames.contains(&frame_id);
                    trace.ready.push((frame_id, ready));
                    match outcome {
                        Outcome::RejectReady => {
                            let _ = reply.send(Err("native frame creation rejected".into()));
                        }
                        Outcome::DisconnectReady => {
                            drop(reply);
                            return trace;
                        }
                        _ => {
                            let _ = reply.send(Ok(ready));
                        }
                    }
                }
                GuiRequest::Prompt {
                    frame_id, reply, ..
                } => {
                    let ready = native_frames.contains(&frame_id);
                    trace.prompts.push((frame_id, ready));
                    let _ = reply.send(if ready {
                        Ok(Some(0))
                    } else {
                        Err("GUI frame is not open".into())
                    });
                }
                GuiRequest::CharacterPalette { frame_id, reply } => {
                    let ready = native_frames.contains(&frame_id);
                    trace.palettes.push((frame_id, ready));
                    let _ = reply.send(if ready {
                        Ok(())
                    } else {
                        Err(format!("GUI frame {frame_id} is not open"))
                    });
                }
                GuiRequest::WarpPointer {
                    frame_id,
                    x,
                    y,
                    absolute,
                    reply,
                } => {
                    assert_eq!(trace.ready.last(), Some(&(frame_id, true)));
                    assert!(native_frames.contains(&frame_id));
                    trace.warps.push((frame_id, x, y, absolute));
                    let _ = reply.send(if matches!(outcome, Outcome::RejectWarp) {
                        Err("native pointer move rejected".into())
                    } else {
                        Ok(())
                    });
                }
                GuiRequest::FrameVisibility { .. } | GuiRequest::FrameOrder { .. } => {}
                _ => panic!("unexpected GUI service in frame readiness test"),
            }
        }
        trace
    });
    (frontend, received_grids, worker)
}

fn finish(
    interp: &mut Interp,
    frontend: &Rc<RefCell<ChanIo>>,
    worker: thread::JoinHandle<Trace>,
) -> Trace {
    interp.set_dialog_backend(None);
    interp.set_color_panel_backend(None);
    interp.set_frame_window_backend(None);
    frontend.borrow_mut().gui_requests = None;
    worker.join().unwrap()
}

fn graphical_interp() -> Interp {
    let mut interp = Interp::new();
    interp
        .eval_str("(set-frame-parameter nil 'window-system 'ns)")
        .unwrap();
    interp
}

#[test]
fn new_frame_is_ready_before_cell_pixel_and_absolute_pointer_requests() {
    let mut interp = graphical_interp();
    let primary = interp.selected_frame.as_ref().unwrap().borrow().id;
    let (frontend, _grids, worker) = attach(&mut interp, Outcome::Success);
    let result = interp.eval_str(
        r#"(let ((frame (make-frame '((window-system . ns) (name . "qa3334-ready")
                       (width . 40) (height . 12) (char-width . 10) (char-height . 20)
                       (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0)
                       (internal-border-width . 3)))))
             (list (set-mouse-position frame 0 0)
                   (set-mouse-pixel-position frame 17 23)
                   (progn (select-frame frame)
                          (set-mouse-absolute-pixel-position 101 203))
                   (condition-case err (set-mouse-position frame 'bad 0)
                     (error (car err)))))"#,
    );
    let trace = finish(&mut interp, &frontend, worker);
    assert_eq!(
        interp.prin1_to_string(&result.unwrap()),
        "(nil nil nil wrong-type-argument)"
    );
    let frame = trace
        .frames
        .iter()
        .find(|(_, name, _)| name == "qa3334-ready")
        .unwrap()
        .0;
    assert!(trace.frames.iter().any(|(id, _, _)| *id == primary));
    assert_eq!(
        trace.ready,
        vec![(frame, false), (frame, true), (frame, true), (frame, true)]
    );
    assert_eq!(
        trace.warps,
        vec![
            (frame, 8, 13, false),
            (frame, 17, 23, false),
            (frame, 101, 203, true)
        ]
    );
    assert!(
        frontend
            .borrow()
            .latest_snapshots
            .iter()
            .any(|snapshot| snapshot.frame_id == frame)
    );
    assert_eq!(trace.snapshots_after_ready, 0);
}

#[test]
fn new_frame_is_ready_before_character_palette_request() {
    let mut interp = graphical_interp();
    let (frontend, _grids, worker) = attach(&mut interp, Outcome::Success);
    let result = interp.eval_str(
        r#"(let ((frame (make-frame '((window-system . ns)
                                      (name . "qa2296-palette")
                                      (width . 40) (height . 12)))))
             (select-frame frame)
             (ns-show-character-palette))"#,
    );
    let trace = finish(&mut interp, &frontend, worker);
    assert!(result.unwrap().is_nil());

    let frame = trace
        .frames
        .iter()
        .find(|(_, name, _)| name == "qa2296-palette")
        .unwrap()
        .0;
    assert_eq!(trace.ready, vec![(frame, false), (frame, true)]);
    assert_eq!(trace.palettes, vec![(frame, true)]);
    assert_eq!(trace.snapshots_after_ready, 0);
}

#[test]
fn preparing_during_a_redisplay_callback_avoids_reborrowing_the_frontend() {
    let mut interp = graphical_interp();
    let (frontend, _grids, worker) = attach(&mut interp, Outcome::Success);
    // The command loop holds this borrow while running pre-redisplay Lisp.
    let mut held = frontend.borrow_mut();
    let result = interp.eval_str(
        r#"(let ((frame (make-frame '((window-system . ns) (name . "qa3334-hidden")
                                      (visibility . nil)))))
             (make-frame-invisible frame t)
             (set-mouse-pixel-position frame 7 9))"#,
    );
    let rendered = held.render(&interp);
    drop(held);
    let trace = finish(&mut interp, &frontend, worker);
    assert!(result.unwrap().is_nil());
    rendered.unwrap();
    let frame = trace
        .frames
        .iter()
        .find(|(_, name, _)| name == "qa3334-hidden")
        .unwrap();
    assert_eq!(frame.2, FrameVisibility::Hidden);
    assert_eq!(trace.warps, vec![(frame.0, 7, 9, false)]);
}

#[test]
fn creation_disconnect_and_pointer_errors_return_to_lisp() {
    let mut interp = graphical_interp();
    for outcome in [
        Outcome::MissingNativeFrame,
        Outcome::RejectReady,
        Outcome::DisconnectReady,
        Outcome::RejectWarp,
    ] {
        let (frontend, _grids, worker) = attach(&mut interp, outcome);
        let result = interp.eval_str(
            r#"(condition-case err
                   (let ((frame (make-frame '((window-system . ns)))))
                     (set-mouse-pixel-position frame 3 5))
                 (error (cadr err)))"#,
        );
        let trace = finish(&mut interp, &frontend, worker);
        let message = match outcome {
            Outcome::MissingNativeFrame => {
                format!("GUI frame {} is not open", trace.ready.last().unwrap().0)
            }
            Outcome::RejectReady => "native frame creation rejected".into(),
            Outcome::DisconnectReady => "GUI window service stopped".into(),
            Outcome::RejectWarp => "native pointer move rejected".into(),
            Outcome::Success => unreachable!(),
        };
        assert_eq!(interp.princ_to_string(&result.unwrap()), message);
        assert_eq!(
            trace.warps.len(),
            usize::from(matches!(outcome, Outcome::RejectWarp))
        );
    }
}


#[test]
fn x_popup_dialog_waits_until_a_new_frame_is_ready() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = graphical_interp();
            let (frontend, _grids, worker) = attach(&mut interp, Outcome::Success);
            let result = interp.eval_str(
                r#"(let ((frame (make-frame '((window-system . ns)
                                              (name . "qa2236-dialog")
                                              (width . 40) (height . 12)))))
                     (x-popup-dialog frame '("Continue?" ("Yes" . yes) ("No" . no))))"#,
            );
            let trace = finish(&mut interp, &frontend, worker);
            let value = result.unwrap_or_else(|flow| match &flow {
                remacs_runtime::lisp::error::Flow::Signal(signal) => panic!(
                    "x-popup-dialog failed: {} {}",
                    interp
                        .prin1_to_string(&signal.symbol())
                        .to_unicode_lossy(),
                    interp
                        .prin1_to_string(&signal.data())
                        .to_unicode_lossy()
                ),
                _ => panic!("x-popup-dialog failed: {flow:?}"),
            });
            assert_eq!(interp.prin1_to_string(&value), "yes");

            let frame_id = trace
                .frames
                .iter()
                .find(|(_, name, _)| name == "qa2236-dialog")
                .unwrap()
                .0;
            assert_eq!(trace.ready, vec![(frame_id, false), (frame_id, true)]);
            assert_eq!(trace.prompts, vec![(frame_id, true)]);
            assert_eq!(trace.snapshots_after_ready, 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn poll_input_preserves_color_drop_for_nested_read_event() {
    let (events, received_events) = mpsc::channel();
    let (grids, _received_grids) = smol::channel::bounded(1);
    let mut io = ChanIo::new_for_test(received_events, grids);
    events
        .send(GuiEvent::ColorDrop {
            position: None,
            color: "112233".to_owned(),
            background: false,
        })
        .unwrap();

    let input = io.poll_input(Duration::ZERO).unwrap();
    assert!(matches!(
        input,
        Some(InputEvent::ColorDrop {
            position: None,
            color,
            background: false,
        }) if color == "112233"
    ));
}
