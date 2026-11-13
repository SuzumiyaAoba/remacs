use remacs_runtime::{
    frontend::FrameWindowBackend,
    lisp::{Interp, Value},
};

struct MockFrameWindowBackend {
    order: Vec<usize>,
}

impl FrameWindowBackend for MockFrameWindowBackend {
    fn set_visible(&self, _frame_id: usize, _visible: bool) -> Result<(), String> {
        Ok(())
    }

    fn raise(&self, _frame_id: usize) -> Result<(), String> {
        Ok(())
    }

    fn lower(&self, _frame_id: usize) -> Result<(), String> {
        Ok(())
    }

    fn frame_list_z_order(&self) -> Result<Vec<usize>, String> {
        Ok(self.order.clone())
    }

    fn warp_pointer_in_frame(
        &self,
        _frame_id: usize,
        _x: i128,
        _y: i128,
    ) -> Result<(), String> {
        Ok(())
    }

    fn warp_pointer_absolute(
        &self,
        _frame_id: usize,
        _x: i128,
        _y: i128,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn ns_frame_list_z_order_returns_the_native_frame_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(require 'frame) (set-frame-parameter nil 'window-system 'ns)")
                .unwrap();
            let selected = interp.selected_frame.as_ref().unwrap().clone();
            let selected_id = selected.borrow().id;
            interp.set_frame_window_backend(Some(std::rc::Rc::new(
                MockFrameWindowBackend {
                    order: vec![selected_id],
                },
            )));

            let result = interp.eval_str("(frame-list-z-order)").unwrap();
            let frames = result.list_to_vec().unwrap();
            assert_eq!(frames.len(), 1);
            assert!(matches!(
                &frames[0],
                Value::Frame(frame) if frame.borrow().id == selected_id
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
