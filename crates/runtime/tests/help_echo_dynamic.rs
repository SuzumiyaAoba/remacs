use remacs_runtime::{
    frontend::{Grid, KeyIo, MousePosition, render_grid_with_mouse_position_cached},
    lisp::Interp,
};
use std::{io, time::Duration};

struct HoverFrontend {
    position: MousePosition,
    grid: Option<Grid>,
}

impl KeyIo for HoverFrontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn unread(&mut self, _: i128) {}

    fn render(&mut self, i: &Interp) -> io::Result<()> {
        self.grid = Some(render_grid_with_mouse_position_cached(
            i,
            40,
            12,
            None,
            Some(self.position),
        ));
        Ok(())
    }

    fn dynamic_help_echo_positions(
        &self,
        i: &Interp,
    ) -> Vec<(remacs_runtime::editor::FrameRef, MousePosition)> {
        i.selected_frame
            .as_ref()
            .map(|frame| vec![(frame.clone(), self.position)])
            .unwrap_or_default()
    }

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (40, 12)
    }
}

fn eval(i: &mut Interp, source: &str) {
    i.eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
}

fn hover(i: &mut Interp, buffer_id: usize) -> remacs_runtime::frontend::Grid {
    let mut frontend = HoverFrontend {
        position: MousePosition {
            buffer_id,
            buffer_position: 0,
            utf16_offset: 0,
            x: 0,
            y: 0,
            row: 0,
            column: 0,
            window_id: None,
            area: None,
            object: None,
            timestamp: 0,
            mode_line_buffer_name: false,
        },
        grid: None,
    };
    frontend.render_mut(i).unwrap();
    frontend.grid.expect("renderer should have captured its grid")
}

fn assert_help_echo(i: &mut Interp, buffer_id: usize, expected: &str) {
    let grid = hover(i, buffer_id);
    assert!(
        grid.help_echoes.iter().any(|region| region.text == expected),
        "expected help echo {expected:?}, got {:?}",
        grid.help_echoes
            .iter()
            .map(|region| &region.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn help_echo_functions_and_expressions_resolve_at_the_hovered_position() {
    std::thread::Builder::new()
        .name("dynamic-help-echo".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let buffer_id = i.current_buffer;

            eval(
                &mut i,
                r#"(progn
                  (erase-buffer)
                  (insert "hover")
                  (setq qa-help-echo-function-calls 0
                        qa-help-echo-function-args nil)
                  (put-text-property
                   (point-min) (point-max) 'help-echo
                   (lambda (window object position)
                     (setq qa-help-echo-function-calls
                           (1+ qa-help-echo-function-calls)
                           qa-help-echo-function-args
                           (list (windowp window)
                                 (eq object (current-buffer))
                                 position))
                     "Dynamic text help")))"#,
            );
            assert_help_echo(&mut i, buffer_id, "Dynamic text help");
            let result = i
                .eval_str("(list qa-help-echo-function-calls qa-help-echo-function-args)")
                .unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(1 (t t 1))"
            );

            eval(
                &mut i,
                r#"(progn
                  (erase-buffer)
                  (insert "hover")
                  (setq qa-help-echo-overlay (make-overlay 1 6)
                        qa-help-echo-overlay-args nil)
                  (overlay-put
                   qa-help-echo-overlay 'help-echo
                   (lambda (window object position)
                     (setq qa-help-echo-overlay-args
                           (list (windowp window)
                                 (eq object qa-help-echo-overlay)
                                 position))
                     "Dynamic overlay help")))"#,
            );
            assert_help_echo(&mut i, buffer_id, "Dynamic overlay help");
            let result = i.eval_str("qa-help-echo-overlay-args").unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(t t 1)"
            );

            eval(
                &mut i,
                r#"(progn
                  (delete-overlay qa-help-echo-overlay)
                  (erase-buffer)
                  (insert "hover")
                  (setq qa-help-echo-expression-calls 0)
                  (put-text-property
                   (point-min) (point-max) 'help-echo
                   '(progn
                      (setq qa-help-echo-expression-calls
                            (1+ qa-help-echo-expression-calls))
                      "Evaluated expression help")))"#,
            );
            assert_help_echo(&mut i, buffer_id, "Evaluated expression help");
            let result = i.eval_str("qa-help-echo-expression-calls").unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "1"
            );
        })
        .unwrap()
        .join()
        .expect("dynamic help-echo test thread panicked");
}
