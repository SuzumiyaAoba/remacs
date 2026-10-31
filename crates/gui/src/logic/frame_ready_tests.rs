use super::*;
use std::{collections::HashSet, thread};

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
        requests, &frontend,
    ))));
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
