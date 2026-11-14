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

    fn update_hover_help(
        &mut self,
        i: &mut Interp,
    ) -> Result<bool, remacs_runtime::lisp::error::Flow> {
        let help = self.grid.as_ref().and_then(|grid| {
            grid.help_echoes
                .iter()
                .find(|region| {
                    region.row == self.position.row
                        && self.position.column >= region.left
                        && self.position.column < region.left.saturating_add(region.width)
                })
        });
        let Some(help) = help else {
            return Ok(false);
        };
        let show_help_function = i.eval_str("(symbol-value 'show-help-function)")?;
        remacs_runtime::frontend::dispatch_hover_help(i, &show_help_function, Some(help))?;
        Ok(true)
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
                        qa-help-echo-function-args nil
                        qa-show-help-calls nil
                        show-help-function
                        (lambda (help)
                          (setq qa-show-help-calls
                                (cons help qa-show-help-calls))))
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
            let result = i.eval_str("qa-show-help-calls").unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(\"Dynamic text help\")"
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
            let result = i.eval_str("qa-show-help-calls").unwrap();
            assert_eq!(
                i.prin1_to_string(&result).to_unicode_lossy(),
                "(\"Dynamic overlay help\" \"Dynamic text help\")"
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

            eval(
                &mut i,
                r#"(progn
                  (erase-buffer)
                  (insert "hover")
                  (setq qa-show-help-calls nil)
                  (put-text-property
                   (point-min) (point-max) 'help-echo "Press \\[next-line]"))"#,
            );
            assert_help_echo(&mut i, buffer_id, r"Press \[next-line]");
            let actual = i.eval_str("qa-show-help-calls").unwrap();
            let expected = i
                .eval_str(r#"(list (substitute-command-keys "Press \\[next-line]"))"#)
                .unwrap();
            assert_eq!(
                i.prin1_to_string(&actual).to_unicode_lossy(),
                i.prin1_to_string(&expected).to_unicode_lossy()
            );

            eval(
                &mut i,
                r#"(progn
                  (erase-buffer)
                  (insert "hover")
                  (setq qa-show-help-calls nil)
                  (put-text-property
                   (point-min) (point-max) 'help-echo
                   (propertize "Press \\[next-line]"
                               'help-echo-inhibit-substitution t)))"#,
            );
            assert_help_echo(&mut i, buffer_id, r"Press \[next-line]");
            let actual = i.eval_str("(car qa-show-help-calls)").unwrap();
            let expected = i.eval_str(r#""Press \\[next-line]""#).unwrap();
            assert_eq!(
                i.prin1_to_string(&actual).to_unicode_lossy(),
                i.prin1_to_string(&expected).to_unicode_lossy()
            );

            eval(&mut i, "(progn (require 'tooltip) (tooltip-mode -1))");
            eval(
                &mut i,
                r#"(progn
                  (erase-buffer)
                  (insert "hover")
                  (put-text-property
                   (point-min) (point-max) 'help-echo "Echo area help"))"#,
            );
            assert_help_echo(&mut i, buffer_id, "Echo area help");
            let message = i.eval_str("(current-message)").unwrap();
            assert_eq!(
                i.prin1_to_string(&message).to_unicode_lossy(),
                "\"Echo area help\""
            );
        })
        .unwrap()
        .join()
        .expect("dynamic help-echo test thread panicked");
}

#[test]
fn hovered_help_uses_tooltip_delay_before_showing_a_tip() {
    std::thread::Builder::new()
        .name("help-echo-tooltip-delay".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let buffer_id = i.current_buffer;
            eval(&mut i, "(require 'tooltip)");
            eval(
                &mut i,
                r#"(progn
                  (message nil)
                  (setq tooltip-mode t
                        tooltip-delay 5
                        tooltip-short-delay 0.1
                        tooltip-timeout-id nil
                        tooltip-hide-time nil
                        show-help-function #'tooltip-show-help
                        qa-tooltip-delay-captured nil)
                  (fset 'run-with-timer
                        (lambda (delay _repeat _function &rest _args)
                          (setq qa-tooltip-delay-captured delay)
                          'qa-tooltip-fake-timer)))"#,
            );
            eval(
                &mut i,
                "(progn (erase-buffer) (insert \"hover\")\
                       (put-text-property 1 6 'help-echo \"Delayed help\"))",
            );
            assert_help_echo(&mut i, buffer_id, "Delayed help");
            assert_eq!(
                i.eval_str("qa-tooltip-delay-captured")
                    .unwrap()
                    .int(),
                Some(5)
            );
        })
        .unwrap()
        .join()
        .expect("tooltip-delay test thread panicked");
}
