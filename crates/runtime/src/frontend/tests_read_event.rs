use super::interp_with_frame;
use super::super::minibuf_loop;
use super::super::super::{InputEvent, KeyIo, MousePosition, MouseWheelDirection};
use crate::lisp::{Interp, Value, eval::MinibufInput, text::EmacsText};
use std::{
    collections::VecDeque,
    io,
    time::Duration,
};

struct WheelFrontend {
    pending: VecDeque<InputEvent>,
}

impl KeyIo for WheelFrontend {
    fn poll_key(&mut self, _dur: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _dur: Duration) -> io::Result<Option<InputEvent>> {
        Ok(self.pending.pop_front())
    }

    fn unread(&mut self, _key: i128) {}

    fn render(&mut self, _i: &Interp) -> io::Result<()> {
        Ok(())
    }

    fn draw_echo(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (20, 5)
    }
}

#[test]
fn single_event_reader_returns_wheel_events_with_position() {
    let mut i = interp_with_frame("");
    let frame = i.selected_frame.as_ref().unwrap().clone();
    let window_id = frame.borrow().selected.borrow().id;
    let position = MousePosition {
        buffer_id: i.current_buffer,
        buffer_position: 0,
        utf16_offset: 0,
        x: 3,
        y: 1,
        row: 0,
        column: 1,
        window_id: Some(window_id),
        area: None,
        mode_line_buffer_name: false,
        object: None,
    };
    let pending = VecDeque::from([
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Up,
            modifiers: 0,
            position,
            line_count: Some(1),
            pixel_delta: Some((0, 5)),
        },
        InputEvent::MouseWheel {
            direction: MouseWheelDirection::Down,
            modifiers: 0,
            position,
            line_count: Some(1),
            pixel_delta: Some((0, -5)),
        },
    ]);
    let term = std::rc::Rc::new(std::cell::RefCell::new(WheelFrontend { pending }));

    for expected in ["wheel-up", "wheel-down"] {
        let MinibufInput::Event(event) =
            minibuf_loop(&term, &mut i, &EmacsText::from(""), true).unwrap()
        else {
            panic!("single event reader must return a wheel event");
        };
        let fields = event.list_to_vec().unwrap();
        assert_eq!(i.prin1_to_string(&fields[0]).to_unicode_lossy(), expected);
        assert!(matches!(&fields[1], Value::Cons(_)), "event position is present");
        assert!(matches!(&fields[3], Value::Int(1)));
        assert!(matches!(&fields[4], Value::Cons(_)), "pixel delta is present");
    }
}
