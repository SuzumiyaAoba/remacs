use super::{EditorView, GuiEvent};
use crate::logic::ChanIo;
use gpui::{Modifiers, MouseButton, point, px};
use remacs_runtime::{
    frontend::{InputEvent, KeyIo, prepare_native_menus, render_grid, run_editor_with_shared},
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, sync::mpsc, time::Duration};

struct Queue(VecDeque<InputEvent>);
impl KeyIo for Queue {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.0
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 12)
    }
}

fn execute(i: &mut Interp, events: Vec<GuiEvent>) {
    let (tx, rx) = mpsc::channel();
    let commands: Vec<_> = events
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                GuiEvent::NativeMenuCommand(_) | GuiEvent::MenuCommand(_)
            )
        })
        .collect();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(commands[0], GuiEvent::NativeMenuCommand(_)),
        "function object must use an ID"
    );
    tx.send(commands.into_iter().next().unwrap()).unwrap();
    let (grid_tx, _grid_rx) = smol::channel::bounded(1);
    let mut bridge = ChanIo::new_for_test(rx, grid_tx);
    let event = bridge
        .poll_input(Duration::ZERO)
        .unwrap()
        .expect("toolbar event");
    assert!(matches!(event, InputEvent::NativeMenuCommand(_)));
    let queue = Rc::new(RefCell::new(Queue(VecDeque::from([event]))));
    assert_eq!(
        run_editor_with_shared(queue, i).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn toolbar_lambda_click_crosses_gui_channel_and_cancels_replaced_binding() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 1)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap) qa-tool-result nil)
        (defun qa-tool-install (initial)
          (define-key tool-bar-map [run]
            (list 'menu-item "Run lambda"
              (eval `(let ((count ,initial))
                (lambda () (interactive)
                  (setq count (1+ count) qa-tool-result count)
                  (message "ran"))) t))))
        (qa-tool-install 7))"#,
    )
    .unwrap();
    prepare_native_menus(&mut i).unwrap();
    let grid = render_grid(&i, 80, 12);
    assert_eq!(grid.tool_bar_items.len(), 1);
    assert!(grid.tool_bar_items[0].command.is_empty());
    assert!(grid.tool_bar_items[0].command_id.is_some());
    let mut app = gpui::TestAppContext::single_with_native_text_system();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|_, cx| EditorView::new(tx, cx));
    handle
        .update(&mut app, |view, window, cx| {
            window.activate_window();
            view.set_internal_border_width(3);
            view.grid = grid;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    rx.try_iter().for_each(drop);
    let bounds = handle
        .update(&mut app, |view, _, _| {
            view.tool_hit_regions.borrow()[0].bounds
        })
        .unwrap();
    let position = point(px(3.0) + bounds.center().x, px(3.0) + bounds.center().y);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), &app);
    visual.simulate_click(position, Modifiers::none());
    execute(&mut i, rx.try_iter().collect());
    assert_eq!(i.eval_str("qa-tool-result").unwrap().int(), Some(8));
    assert_eq!(i.echo_message, "ran");

    visual.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
    assert!(
        !rx.try_iter()
            .any(|event| matches!(event, GuiEvent::NativeMenuCommand(_)))
    );
    i.eval_str("(qa-tool-install 100)").unwrap();
    prepare_native_menus(&mut i).unwrap();
    let next = render_grid(&i, 80, 12);
    handle
        .update(&mut app, |view, _, cx| {
            view.grid = next;
            cx.notify();
        })
        .unwrap();
    app.run_until_parked();
    visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    assert!(
        !rx.try_iter().any(|event| matches!(
            event,
            GuiEvent::NativeMenuCommand(_) | GuiEvent::MenuCommand(_)
        )),
        "releasing an old press must not run its replacement"
    );
    visual.simulate_click(position, Modifiers::none());
    execute(&mut i, rx.try_iter().collect());
    assert_eq!(i.eval_str("qa-tool-result").unwrap().int(), Some(101));
}
