use remacs_runtime::{
    frontend::{ApplicationVisibilityBackend, UserAttentionUrgency},
    lisp::Interp,
};
use std::{cell::RefCell, rc::Rc, thread};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Hide,
    Activate,
    Unhide,
    HideOtherApps,
    ActivateUnhidden { has_frame: bool, front: bool },
}

#[derive(Clone)]
struct RecordingBackend(Rc<RefCell<Vec<Call>>>);

impl ApplicationVisibilityBackend for RecordingBackend {
    fn hide(&self) -> Result<(), String> {
        self.0.borrow_mut().push(Call::Hide);
        Ok(())
    }

    fn activate(&self) -> Result<(), String> {
        self.0.borrow_mut().push(Call::Activate);
        Ok(())
    }

    fn unhide(&self) -> Result<(), String> {
        self.0.borrow_mut().push(Call::Unhide);
        Ok(())
    }

    fn activate_unhidden(
        &self,
        frame_id: Option<usize>,
        bring_frame_to_front: bool,
    ) -> Result<(), String> {
        self.0.borrow_mut().push(Call::ActivateUnhidden {
            has_frame: frame_id.is_some(),
            front: bring_frame_to_front,
        });
        Ok(())
    }

    fn hide_other_apps(&self) -> Result<(), String> {
        self.0.borrow_mut().push(Call::HideOtherApps);
        Ok(())
    }

    fn set_dock_badge(&self, _badge: Option<&str>) -> Result<(), String> {
        Ok(())
    }

    fn set_dock_progress(&self, _progress: Option<f64>) -> Result<(), String> {
        Ok(())
    }

    fn request_user_attention(&self, _urgency: Option<UserAttentionUrgency>) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn ns_hide_emacs_activation_symbols_keep_their_distinct_modes() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'ns)")
                .unwrap();
            let calls = Rc::new(RefCell::new(Vec::new()));
            interp
                .set_application_visibility_backend(Some(Rc::new(RecordingBackend(calls.clone()))));

            let forms = [
                "(ns-hide-emacs 'activate)",
                "(ns-hide-emacs 'activate-front)",
                "(ns-hide-emacs nil)",
                "(ns-hide-emacs t)",
            ];
            if cfg!(target_os = "macos") {
                for form in forms {
                    interp.eval_str(form).unwrap();
                }
                assert_eq!(
                    *calls.borrow(),
                    vec![
                        Call::ActivateUnhidden {
                            has_frame: true,
                            front: false,
                        },
                        Call::ActivateUnhidden {
                            has_frame: true,
                            front: true,
                        },
                        Call::Unhide,
                        Call::Hide,
                    ]
                );
            } else {
                for form in forms {
                    assert!(interp.eval_str(form).is_err());
                }
                assert!(calls.borrow().is_empty());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ns_hide_others_uses_the_backend_only_on_macos() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let calls = Rc::new(RefCell::new(Vec::new()));
            interp
                .set_application_visibility_backend(Some(Rc::new(RecordingBackend(calls.clone()))));

            let result = interp.eval_str("(ns-hide-others)");
            if cfg!(target_os = "macos") {
                result.unwrap();
                assert_eq!(*calls.borrow(), vec![Call::HideOtherApps]);
            } else {
                assert!(result.is_err());
                assert!(calls.borrow().is_empty());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
