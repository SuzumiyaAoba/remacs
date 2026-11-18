use remacs_runtime::{
    frontend::{KeyIo, install_minibuf_reader},
    lisp::{Flow, Interp, Value},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    rc::Rc,
    time::Duration,
};

struct HeadlessFrontend {
    pending: Rc<RefCell<VecDeque<i128>>>,
    renders: Rc<Cell<usize>>,
}

fn eval(interp: &mut Interp, source: &str) -> Value {
    match interp.eval_str(source) {
        Ok(value) => value,
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(flow) => panic!("{flow:?}"),
    }
}

impl KeyIo for HeadlessFrontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(self.pending.borrow_mut().pop_front())
    }

    fn unread(&mut self, _: i128) {}

    fn render(&mut self, _: &Interp) -> io::Result<()> {
        self.renders.set(self.renders.get() + 1);
        Ok(())
    }

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (80, 25)
    }
}

#[test]
fn zero_duration_sit_for_does_not_fire_due_timers() {
    let mut interp = Interp::new();
    let frontend = Rc::new(RefCell::new(HeadlessFrontend {
        pending: Rc::new(RefCell::new(VecDeque::new())),
        renders: Rc::new(Cell::new(0)),
    }));
    install_minibuf_reader(&frontend, &mut interp);

    let zero_wait = eval(
        &mut interp,
        r#"(let ((timer-list nil) (timer-idle-list nil) (hits 0))
              (unwind-protect
                  (progn
                    (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
                      (if (sit-for 0) hits -1))
                (mapc #'cancel-timer timer-list)
                (mapc #'cancel-timer timer-idle-list)))"#,
    );
    assert_eq!(zero_wait.int(), Some(0));

    let positive_wait = eval(
        &mut interp,
        r#"(let ((timer-list nil) (timer-idle-list nil) (hits 0))
              (unwind-protect
                  (progn
                    (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
                    (if (sit-for 0.02) hits -1))
                (mapc #'cancel-timer timer-list)
                (mapc #'cancel-timer timer-idle-list)))"#,
    );
    assert_eq!(positive_wait.int(), Some(1));
}

#[test]
fn sit_for_nodisp_suppresses_frontend_redisplay() {
    let mut interp = Interp::new();
    let renders = Rc::new(Cell::new(0));
    let frontend = Rc::new(RefCell::new(HeadlessFrontend {
        pending: Rc::new(RefCell::new(VecDeque::new())),
        renders: renders.clone(),
    }));
    install_minibuf_reader(&frontend, &mut interp);

    let before = renders.get();
    assert!(eval(&mut interp, "(sit-for 0 t)").truthy());
    assert_eq!(renders.get(), before);

    let positive_wait = eval(
        &mut interp,
        r#"(let ((timer-list nil) (timer-idle-list nil) (hits 0))
              (unwind-protect
                  (progn
                    (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
                    (if (sit-for 0.02 t) hits -1))
                (mapc #'cancel-timer timer-list)
                (mapc #'cancel-timer timer-idle-list)))"#,
    );
    assert_eq!(positive_wait.int(), Some(1));
    assert_eq!(renders.get(), before);

    assert!(eval(&mut interp, "(sit-for 0)").truthy());
    assert!(renders.get() > before);
}

#[test]
fn sit_for_skips_redisplay_when_input_is_already_pending() {
    let mut interp = Interp::new();
    let renders = Rc::new(Cell::new(0));
    let frontend = Rc::new(RefCell::new(HeadlessFrontend {
        pending: Rc::new(RefCell::new(VecDeque::from(['x' as i128]))),
        renders: renders.clone(),
    }));
    install_minibuf_reader(&frontend, &mut interp);

    let result = eval(&mut interp, "(sit-for 10)");
    assert!(result.is_nil());
    assert_eq!(renders.get(), 0);
    assert_eq!(eval(&mut interp, "(read-event)").int(), Some('x' as i128));

    let unread_result = eval(
        &mut interp,
        "(let ((unread-command-events '(120))) (if (and (null (sit-for 10)) (eq (read-event) 120)) 1 0))",
    );
    assert_eq!(unread_result.int(), Some(1));
    assert_eq!(renders.get(), 0);
}
