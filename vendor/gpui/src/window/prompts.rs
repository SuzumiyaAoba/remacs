use std::ops::Deref;

use futures::channel::oneshot;

use crate::{
    AnyView, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, PromptButton, PromptLevel,
    Render, ScrollHandle, StatefulInteractiveElement, Styled, div, opaque_grey, px, white,
};

use super::Window;

/// The event emitted when a prompt's option is selected.
/// The usize is the index of the selected option, from the actions
/// passed to the prompt.
pub struct PromptResponse(pub usize);

/// A prompt that can be rendered in the window.
pub trait Prompt: EventEmitter<PromptResponse> + Focusable {}

impl<V: EventEmitter<PromptResponse> + Focusable> Prompt for V {}

/// A handle to a prompt that can be used to interact with it.
pub struct PromptHandle {
    sender: oneshot::Sender<usize>,
}

impl PromptHandle {
    pub(crate) fn new(sender: oneshot::Sender<usize>) -> Self {
        Self { sender }
    }

    /// Construct a new prompt handle from a view of the appropriate types
    pub fn with_view<V: Prompt + Render>(
        self,
        view: Entity<V>,
        window: &mut Window,
        cx: &mut App,
    ) -> RenderablePromptHandle {
        let mut sender = Some(self.sender);
        let previous_focus = window.focused(cx);
        let window_handle = window.window_handle();
        cx.subscribe(&view, move |_: Entity<V>, e: &PromptResponse, cx| {
            if let Some(sender) = sender.take() {
                sender.send(e.0).ok();
                window_handle
                    .update(cx, |_, window, _cx| {
                        window.prompt.take();
                        if let Some(previous_focus) = &previous_focus {
                            window.focus(previous_focus);
                        }
                    })
                    .ok();
            }
        })
        .detach();

        window.focus(&view.focus_handle(cx));

        RenderablePromptHandle {
            view: Box::new(view),
        }
    }
}

/// A prompt handle capable of being rendered in a window.
pub struct RenderablePromptHandle {
    pub(crate) view: Box<dyn PromptViewHandle>,
}

/// Use this function in conjunction with [App::set_prompt_builder] to force
/// GPUI to always use the fallback prompt renderer.
pub fn fallback_prompt_renderer(
    level: PromptLevel,
    message: &str,
    detail: Option<&str>,
    actions: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
) -> RenderablePromptHandle {
    let previous_focus = window.focused(cx);
    let renderer = cx.new(|cx| FallbackPromptRenderer {
        level,
        message: message.to_string(),
        detail: detail.map(ToString::to_string),
        actions: actions.to_vec(),
        action_focus: actions.iter().map(|_| cx.focus_handle()).collect(),
        selected: actions
            .iter()
            .position(PromptButton::is_selectable)
            .unwrap_or(0),
        scroll: ScrollHandle::new(),
        last_focused: actions.iter().position(PromptButton::is_selectable),
        previous_focus,
        focus: cx.focus_handle(),
    });

    handle.with_view(renderer, window, cx)
}

/// The default GPUI fallback for rendering prompts, when the platform doesn't support it.
pub struct FallbackPromptRenderer {
    level: PromptLevel,
    message: String,
    detail: Option<String>,
    actions: Vec<PromptButton>,
    action_focus: Vec<FocusHandle>,
    selected: usize,
    scroll: ScrollHandle,
    last_focused: Option<usize>,
    previous_focus: Option<FocusHandle>,
    focus: FocusHandle,
}

impl FallbackPromptRenderer {
    fn action_rows(&self) -> (bool, Vec<[Option<usize>; 2]>) {
        let split = self
            .actions
            .iter()
            .position(PromptButton::is_separator)
            .or_else(|| {
                self.level
                    .dialog_title()
                    .map(|_| self.actions.len().div_ceil(2))
            });
        let indices = self
            .actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| (!action.is_separator()).then_some(index));
        if let Some(split) = split {
            let (left, right): (Vec<usize>, Vec<usize>) = indices.partition(|index| *index < split);
            let rows = (0..left.len().max(right.len()))
                .map(|row| [left.get(row).copied(), right.get(row).copied()])
                .collect();
            (true, rows)
        } else {
            (false, indices.map(|index| [Some(index), None]).collect())
        }
    }

    fn scroll_index_for_action(&self, index: usize) -> Option<usize> {
        self.action_rows()
            .1
            .iter()
            .position(|row| row.contains(&Some(index)))
            .map(|row| row + 1 + usize::from(self.detail.is_some()))
    }

    fn scroll_to_action(&self, index: usize) {
        if let Some(row) = self.scroll_index_for_action(index) {
            self.scroll.scroll_to_item(row);
        }
    }

    fn render_action(
        &self,
        index: usize,
        window: &Window,
        cx: &Context<Self>,
    ) -> crate::AnyElement {
        let action = &self.actions[index];
        let row = div()
            .w_full()
            .flex()
            .flex_shrink_0()
            .flex_row()
            .justify_around()
            .text_sm()
            .child(action.label().clone());
        if !action.is_selectable() {
            let text = row.justify_start().cursor_default();
            #[cfg(any(test, feature = "test-support"))]
            let text = text.debug_selector(|| "fallback-prompt-text".to_string());
            return text.into_any_element();
        }
        let button = row
            .border_1()
            .border_color(if self.action_focus[index].is_focused(window) {
                opaque_grey(0.1, 1.0)
            } else {
                opaque_grey(0.2, 0.5)
            })
            .rounded_xs()
            .cursor_pointer()
            .id(index)
            .track_focus(&self.action_focus[index])
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(PromptResponse(index));
            }));
        #[cfg(any(test, feature = "test-support"))]
        let button = button.debug_selector(move || format!("fallback-prompt-action-{index}"));
        button.into_any_element()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.prevent_default();
        let key = &event.keystroke;
        if key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.platform
            || key.modifiers.function
            || key.modifiers.hyper
        {
            return;
        }
        if let Some(selected) = self
            .action_focus
            .iter()
            .position(|focus| focus.is_focused(window))
        {
            self.selected = selected;
        }
        match key.key.as_str() {
            "tab" if self.actions.iter().any(PromptButton::is_selectable) => {
                for _ in 0..self.actions.len() {
                    self.selected = if key.modifiers.shift {
                        if self.selected == 0 {
                            self.actions.len() - 1
                        } else {
                            self.selected - 1
                        }
                    } else {
                        (self.selected + 1) % self.actions.len()
                    };
                    if self.actions[self.selected].is_selectable() {
                        break;
                    }
                }
                window.focus(&self.action_focus[self.selected]);
                self.scroll_to_action(self.selected);
                cx.notify();
            }
            "enter" | "space"
                if self
                    .actions
                    .get(self.selected)
                    .is_some_and(PromptButton::is_selectable) =>
            {
                cx.emit(PromptResponse(self.selected));
            }
            "escape" => {
                if let Some(index) = self.actions.iter().position(PromptButton::is_cancel) {
                    cx.emit(PromptResponse(index));
                } else {
                    // Dropping the prompt closes the response channel without
                    // fabricating a selected button.
                    window.prompt.take();
                    if let Some(focus) = &self.previous_focus {
                        window.focus(focus);
                    } else {
                        window.blur();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Render for FallbackPromptRenderer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self
            .action_focus
            .iter()
            .position(|focus| focus.is_focused(window));
        if focused != self.last_focused {
            self.last_focused = focused;
            if let Some(index) = focused {
                // Follow any focus change while allowing independent wheel scrolling.
                self.scroll_to_action(index);
            }
        }
        let (grouped, rows) = self.action_rows();
        let content = div()
            .id("gpui-fallback-prompt-content")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .min_h_0()
            .flex()
            .flex_col()
            .flex_shrink()
            .overflow_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .justify_around()
                    .child(div().overflow_hidden().child(self.message.clone())),
            )
            .children(self.detail.clone().map(|detail| {
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .justify_around()
                    .text_sm()
                    .mb_2()
                    .child(div().child(detail))
            }))
            .children(rows.into_iter().map(|[left, right]| {
                let row = div()
                    .w_full()
                    .flex()
                    .flex_shrink_0()
                    .flex_row()
                    .gap_2()
                    .mt_1();
                if grouped {
                    row.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .children(left.map(|index| self.render_action(index, window, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .children(right.map(|index| self.render_action(index, window, cx))),
                    )
                } else {
                    row.children(left.map(|index| self.render_action(index, window, cx)))
                }
            }));

        let prompt = div()
            .id("gpui-fallback-prompt")
            .cursor_default()
            .w_72()
            .max_h((window.viewport_size().height - px(24.0)).max(px(0.0)))
            .flex()
            .flex_col()
            .bg(white())
            .rounded_lg()
            .overflow_hidden()
            .p_3()
            .children(self.level.dialog_title().map(|title| {
                let heading = div().w_full().flex_shrink_0().text_sm().mb_2().child(title);
                #[cfg(any(test, feature = "test-support"))]
                let heading = heading.debug_selector(|| "fallback-dialog-title".to_string());
                heading
            }))
            .child(content);

        div()
            .size_full()
            .child(
                div()
                    .size_full()
                    .bg(opaque_grey(0.5, 0.6))
                    .absolute()
                    .top_0()
                    .left_0(),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .flex_col()
                    .justify_around()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .justify_around()
                            .child(prompt),
                    ),
            )
    }
}

impl EventEmitter<PromptResponse> for FallbackPromptRenderer {}

impl Focusable for FallbackPromptRenderer {
    fn focus_handle(&self, _: &crate::App) -> FocusHandle {
        self.actions
            .iter()
            .position(PromptButton::is_selectable)
            .map(|index| &self.action_focus[index])
            .unwrap_or(&self.focus)
            .clone()
    }
}

pub(crate) trait PromptViewHandle {
    fn any_view(&self) -> AnyView;
}

impl<V: Prompt + Render> PromptViewHandle for Entity<V> {
    fn any_view(&self) -> AnyView {
        self.clone().into()
    }
}

pub(crate) enum PromptBuilder {
    Default,
    Custom(
        Box<
            dyn Fn(
                PromptLevel,
                &str,
                Option<&str>,
                &[PromptButton],
                PromptHandle,
                &mut Window,
                &mut App,
            ) -> RenderablePromptHandle,
        >,
    ),
}

impl Deref for PromptBuilder {
    type Target = dyn Fn(
        PromptLevel,
        &str,
        Option<&str>,
        &[PromptButton],
        PromptHandle,
        &mut Window,
        &mut App,
    ) -> RenderablePromptHandle;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Default => &fallback_prompt_renderer,
            Self::Custom(f) => f.as_ref(),
        }
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::*;
    use crate::{
        Bounds, Modifiers, Pixels, ScrollDelta, ScrollWheelEvent, TestAppContext,
        VisualTestContext, point, size,
    };

    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    fn prompt(
        count: usize,
        detail: bool,
    ) -> (
        VisualTestContext,
        Entity<FallbackPromptRenderer>,
        oneshot::Receiver<usize>,
    ) {
        prompt_with_level(count, detail, PromptLevel::Info)
    }

    fn prompt_with_level(
        count: usize,
        detail: bool,
        level: PromptLevel,
    ) -> (
        VisualTestContext,
        Entity<FallbackPromptRenderer>,
        oneshot::Receiver<usize>,
    ) {
        let actions: Vec<_> = (0..count)
            .map(|index| PromptButton::new(format!("Option {index}")))
            .collect();
        prompt_actions(actions, detail, level)
    }

    fn prompt_actions(
        actions: Vec<PromptButton>,
        detail: bool,
        level: PromptLevel,
    ) -> (
        VisualTestContext,
        Entity<FallbackPromptRenderer>,
        oneshot::Receiver<usize>,
    ) {
        let mut app = TestAppContext::single();
        app.update(|app| app.set_prompt_builder(fallback_prompt_renderer));
        let window = app.add_window(|_, _| Empty);
        let mut cx = VisualTestContext::from_window(*window, &app);
        cx.simulate_resize(size(px(400.0), px(300.0)));
        let response = cx.update(|window, app| {
            window.prompt(
                level,
                "Choose one",
                detail.then_some("More details"),
                &actions,
                app,
            )
        });
        cx.run_until_parked();
        let renderer = cx.update(|window, _| {
            window
                .prompt
                .as_ref()
                .unwrap()
                .view
                .any_view()
                .downcast::<FallbackPromptRenderer>()
                .ok()
                .unwrap()
        });
        (cx, renderer, response)
    }

    fn scroll(
        cx: &mut VisualTestContext,
        renderer: &Entity<FallbackPromptRenderer>,
    ) -> ScrollHandle {
        cx.update(|_, app| renderer.read(app).scroll.clone())
    }

    fn visible_child(scroll: &ScrollHandle, index: usize) -> Bounds<Pixels> {
        let mut child = scroll.bounds_for_item(index).expect("laid out button");
        child.origin += scroll.offset();
        assert!(
            child.top() >= scroll.bounds().top(),
            "button top is visible: {child:?}"
        );
        assert!(
            child.bottom() <= scroll.bounds().bottom(),
            "button bottom is visible: {child:?}"
        );
        child
    }

    #[test]
    fn fallback_long_prompt_scrolls_to_the_last_mouse_choice() {
        let (mut cx, renderer, mut response) = prompt(50, false);
        let scroll = scroll(&mut cx, &renderer);
        assert!(scroll.bounds().top() >= px(0.0));
        assert!(scroll.bounds().bottom() <= px(300.0));
        assert!(scroll.max_offset().height > px(0.0));
        cx.simulate_event(ScrollWheelEvent {
            position: scroll.bounds().center(),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-100000.0))),
            ..Default::default()
        });
        assert!(scroll.offset().y < px(0.0));
        let last = visible_child(&scroll, 50);
        cx.simulate_click(last.center(), Modifiers::default());
        assert_eq!(response.try_recv().unwrap(), Some(49));
    }

    #[test]
    fn fallback_long_prompt_scrolls_the_keyboard_focus_into_view() {
        let (mut cx, renderer, mut response) = prompt(50, true);
        cx.simulate_keystrokes("shift-tab");
        let scroll = scroll(&mut cx, &renderer);
        assert!(scroll.offset().y < px(0.0));
        visible_child(&scroll, 51);
        cx.simulate_keystrokes("enter");
        assert_eq!(response.try_recv().unwrap(), Some(49));
    }

    #[test]
    fn fallback_long_prompt_scrolls_independently_changed_focus_into_view() {
        let (mut cx, renderer, mut response) = prompt(50, false);
        cx.update(|window, app| window.focus(&renderer.read(app).action_focus[49]));
        cx.run_until_parked();
        let scroll = scroll(&mut cx, &renderer);
        visible_child(&scroll, 50);
        cx.simulate_keystrokes("enter");
        assert_eq!(response.try_recv().unwrap(), Some(49));
    }

    #[test]
    fn fallback_prompt_resizes_its_scroll_viewport() {
        let (mut cx, renderer, mut response) = prompt(50, false);
        cx.simulate_resize(size(px(400.0), px(180.0)));
        let scroll = scroll(&mut cx, &renderer);
        assert!(scroll.bounds().top() >= px(0.0));
        assert!(scroll.bounds().bottom() <= px(180.0));
        cx.simulate_event(ScrollWheelEvent {
            position: scroll.bounds().center(),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-100000.0))),
            ..Default::default()
        });
        let last = visible_child(&scroll, 50);
        cx.simulate_click(last.center(), Modifiers::default());
        assert_eq!(response.try_recv().unwrap(), Some(49));
    }

    #[test]
    fn explicit_dialog_headings_stay_visible_while_choices_scroll() {
        for level in [PromptLevel::Question, PromptLevel::Information] {
            let (mut cx, renderer, mut response) = prompt_with_level(50, true, level);
            let heading = cx
                .debug_bounds("fallback-dialog-title")
                .expect("rendered heading");
            let scroll = scroll(&mut cx, &renderer);
            assert!(heading.size.height > px(0.0));
            assert!(heading.bottom() <= scroll.bounds().top());
            cx.simulate_keystrokes("shift-tab");
            assert_eq!(cx.debug_bounds("fallback-dialog-title"), Some(heading));
            let row = cx.update(|_, app| renderer.read(app).scroll_index_for_action(49).unwrap());
            visible_child(&scroll, row);
            cx.simulate_keystrokes("enter");
            assert_eq!(response.try_recv().unwrap(), Some(49));
        }
    }

    #[test]
    fn group_boundary_places_buttons_on_opposite_sides_and_preserves_mouse_result() {
        let (mut cx, _, mut response) = prompt_actions(
            vec![
                PromptButton::new("Save"),
                PromptButton::separator(),
                PromptButton::new("Discard"),
                PromptButton::cancel("Cancel"),
            ],
            false,
            PromptLevel::Question,
        );
        let left = cx.debug_bounds("fallback-prompt-action-0").unwrap();
        let right = cx.debug_bounds("fallback-prompt-action-2").unwrap();
        let cancel = cx.debug_bounds("fallback-prompt-action-3").unwrap();
        assert!(left.right() < right.left());
        assert_eq!(left.top(), right.top());
        assert_eq!(right.left(), cancel.left());
        assert!(right.bottom() <= cancel.top());
        cx.simulate_click(cancel.center(), Modifiers::default());
        assert_eq!(response.try_recv().unwrap(), Some(3));
    }

    #[test]
    fn default_groups_use_the_larger_left_half_and_repeated_boundaries_keep_the_first_split() {
        let (mut cx, _, _) = prompt_actions(
            vec![
                PromptButton::new("A"),
                PromptButton::new("B"),
                PromptButton::new("C"),
            ],
            false,
            PromptLevel::Information,
        );
        let a = cx.debug_bounds("fallback-prompt-action-0").unwrap();
        let b = cx.debug_bounds("fallback-prompt-action-1").unwrap();
        let c = cx.debug_bounds("fallback-prompt-action-2").unwrap();
        assert_eq!(a.left(), b.left());
        assert!(a.right() < c.left());
        assert_eq!(a.top(), c.top());
        let (mut cx, _, _) = prompt_actions(
            vec![
                PromptButton::new("A"),
                PromptButton::separator(),
                PromptButton::new("B"),
                PromptButton::separator(),
                PromptButton::new("C"),
            ],
            false,
            PromptLevel::Info,
        );
        let a = cx.debug_bounds("fallback-prompt-action-0").unwrap();
        let b = cx.debug_bounds("fallback-prompt-action-2").unwrap();
        let c = cx.debug_bounds("fallback-prompt-action-4").unwrap();
        assert!(a.right() < b.left());
        assert_eq!(b.left(), c.left());
    }

    #[test]
    fn empty_left_or_right_groups_retain_the_requested_side() {
        for (actions, index, right) in [
            (
                vec![
                    PromptButton::separator(),
                    PromptButton::new("A"),
                    PromptButton::new("B"),
                ],
                1,
                true,
            ),
            (
                vec![
                    PromptButton::new("A"),
                    PromptButton::new("B"),
                    PromptButton::separator(),
                ],
                0,
                false,
            ),
        ] {
            let (mut cx, renderer, _) = prompt_actions(actions, false, PromptLevel::Question);
            let bounds = if index == 1 {
                cx.debug_bounds("fallback-prompt-action-1").unwrap()
            } else {
                cx.debug_bounds("fallback-prompt-action-0").unwrap()
            };
            let viewport = scroll(&mut cx, &renderer).bounds();
            assert_eq!(bounds.center().x > viewport.center().x, right);
        }
    }

    #[test]
    fn long_groups_scroll_the_focused_or_clicked_button_into_view() {
        for keyboard in [true, false] {
            let mut actions: Vec<_> = (0..50)
                .map(|index| PromptButton::new(format!("Option {index}")))
                .collect();
            actions.insert(5, PromptButton::separator());
            let (mut cx, renderer, mut response) =
                prompt_actions(actions, true, PromptLevel::Question);
            let viewport = scroll(&mut cx, &renderer);
            if keyboard {
                cx.simulate_keystrokes("shift-tab");
            } else {
                cx.simulate_event(ScrollWheelEvent {
                    position: viewport.bounds().center(),
                    delta: ScrollDelta::Pixels(point(px(0.0), px(-100000.0))),
                    ..Default::default()
                });
            }
            let last = cx.debug_bounds("fallback-prompt-action-50").unwrap();
            assert!(last.top() >= viewport.bounds().top());
            assert!(last.bottom() <= viewport.bounds().bottom());
            if keyboard {
                cx.simulate_keystrokes("enter");
            } else {
                cx.simulate_click(last.center(), Modifiers::default());
            }
            assert_eq!(response.try_recv().unwrap(), Some(50));
        }
    }
}
