use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, PromptLevel, Render, Styled,
    TestAppContext, Window, WindowHandle, div,
};
use std::{cell::Cell, rc::Rc};

struct PromptRoot {
    focus: FocusHandle,
    keys_received: usize,
}

impl Render for PromptRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, _, _, _| this.keys_received += 1))
    }
}

type Response = Rc<Cell<Option<Result<usize, ()>>>>;

fn prompt(
    labels: &[&str],
) -> (
    TestAppContext,
    WindowHandle<PromptRoot>,
    Response,
    FocusHandle,
) {
    let mut app = TestAppContext::single();
    app.update(|app| app.set_prompt_builder(gpui::fallback_prompt_renderer));
    let window = app.add_window(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus);
        PromptRoot {
            focus,
            keys_received: 0,
        }
    });
    let previous_focus = window
        .update(&mut app, |root, _, _| root.focus.clone())
        .unwrap();
    let buttons =
        crate::dialog_prompt_buttons(labels.iter().map(|label| (*label).into()).collect());
    let receiver = window
        .update(&mut app, |_, window, app| {
            window.prompt(PromptLevel::Info, "Choose", None, &buttons, app)
        })
        .unwrap();
    let response = Rc::new(Cell::new(None));
    let outcome = response.clone();
    app.update(|app| {
        app.spawn(async move |_| {
            outcome.set(Some(receiver.await.map_err(|_| ())));
        })
        .detach();
    });
    app.background_executor.run_until_parked();
    (app, window, response, previous_focus)
}

fn restored(app: &mut TestAppContext, window: WindowHandle<PromptRoot>, focus: &FocusHandle) {
    window
        .update(app, |root, window, _| {
            assert!(
                focus.is_focused(window),
                "restore focus to the original view"
            );
            assert_eq!(
                root.keys_received, 0,
                "prompt keys must not reach the original view"
            );
        })
        .unwrap();
}

#[test]
fn fallback_prompt_enter_accepts_the_first_button() {
    let (mut app, window, response, focus) = prompt(&["Continue", "Cancel"]);
    app.simulate_keystrokes(*window, "enter");
    assert_eq!(response.get(), Some(Ok(0)));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_tab_moves_focus_and_shift_tab_wraps() {
    let (mut app, window, response, focus) = prompt(&["One", "Two", "Cancel"]);
    let first = window
        .update(&mut app, |_, window, cx| window.focused(cx).unwrap())
        .unwrap();
    app.simulate_keystrokes(*window, "tab");
    let second = window
        .update(&mut app, |_, window, cx| window.focused(cx).unwrap())
        .unwrap();
    assert_ne!(first, second, "Tab must focus another button");
    app.simulate_keystrokes(*window, "shift-tab");
    window
        .update(&mut app, |_, window, _| assert!(first.is_focused(window)))
        .unwrap();
    app.simulate_keystrokes(*window, "shift-tab enter");
    assert_eq!(response.get(), Some(Ok(2)));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_tab_wraps_forward_and_space_accepts() {
    let (mut app, window, response, focus) = prompt(&["One", "Two", "Three"]);
    app.simulate_keystrokes(*window, "tab tab tab tab tab space");
    assert_eq!(response.get(), Some(Ok(2)));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_accepts_the_button_focused_outside_key_navigation() {
    let (mut app, window, response, focus) = prompt(&["One", "Two", "Three"]);
    app.simulate_keystrokes(*window, "tab");
    let second = window
        .update(&mut app, |_, window, cx| window.focused(cx).unwrap())
        .unwrap();
    app.simulate_keystrokes(*window, "shift-tab");
    // Mouse focus and accessibility focus can change independently of Tab.
    window
        .update(&mut app, |_, window, _| window.focus(&second))
        .unwrap();
    app.simulate_keystrokes(*window, "enter");
    assert_eq!(response.get(), Some(Ok(1)));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_escape_selects_the_cancel_button() {
    let (mut app, window, response, focus) = prompt(&["Continue", "Cancel"]);
    app.simulate_keystrokes(*window, "escape");
    assert_eq!(response.get(), Some(Ok(1)));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_escape_without_cancel_dismisses_without_selection() {
    let (mut app, window, response, focus) = prompt(&["One", "Two"]);
    app.simulate_keystrokes(*window, "escape");
    assert_eq!(response.get(), Some(Err(())));
    restored(&mut app, window, &focus);
}

#[test]
fn fallback_prompt_empty_actions_still_allow_dismissal() {
    let (mut app, window, response, focus) = prompt(&[]);
    app.simulate_keystrokes(*window, "tab shift-tab enter space");
    assert_eq!(response.get(), None);
    app.simulate_keystrokes(*window, "escape");
    assert_eq!(response.get(), Some(Err(())));
    restored(&mut app, window, &focus);
}
