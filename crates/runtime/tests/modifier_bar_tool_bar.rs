use remacs_runtime::{
    frontend::{
        InputEvent, KeyIo, NativeMenuCommandId, prepare_native_menus, render_grid, run_editor_with,
    },
    lisp::Interp,
};
use std::{collections::VecDeque, io, path::PathBuf, time::Duration};

struct ModifierBarFrontend {
    events: VecDeque<InputEvent>,
}

impl KeyIo for ModifierBarFrontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.events.pop_front().map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "modifier-bar input exhausted")
        })
    }

    fn unread(&mut self, _: i128) {}

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (120, 24)
    }

    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn modifier_bar_items_render_on_a_secondary_toolbar_row() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let lisp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../lisp")
                .canonicalize()
                .unwrap();
            i.eval_str(&format!(
                "(setq load-path (cons {:?} load-path))",
                lisp_dir.to_string_lossy()
            ))
            .unwrap();
            i.eval_str(
                r#"(progn
                  (require 'tool-bar)
                  (set-frame-parameter nil 'window-system 'ns)
                  (set-frame-parameter nil 'menu-bar-lines 0)
                  (set-frame-parameter nil 'tab-bar-lines 0)
                  (setq tool-bar-map (make-sparse-keymap))
                  (defun qa-modifier-bar-primary () (interactive))
                  (setq qa-modifier-control-seen nil)
                  (global-set-key (kbd "C-a")
                    (lambda () (interactive) (setq qa-modifier-control-seen t)))
                  (define-key tool-bar-map [primary]
                    '(menu-item "Primary" qa-modifier-bar-primary))
                  (tool-bar-mode 1)
                  (modifier-bar-mode 1)
                  (set-frame-parameter nil 'tool-bar-lines 1))"#,
            )
            .unwrap();
            let state = i
                .eval_str(
                    "(list (fboundp 'tool-bar-make-keymap) (frame-parameter nil 'tool-bar-position) (and (boundp 'secondary-tool-bar-map) (keymapp secondary-tool-bar-map)))",
                )
                .unwrap();
            prepare_native_menus(&mut i).unwrap();
            let grid = render_grid(&i, 120, 24);
            let labels = grid
                .tool_bar_items
                .iter()
                .map(|item| {
                    let (start, end) = item.byte_range.unwrap();
                    (
                        grid.rows[item.row][start..end].trim().to_owned(),
                        item.row,
                        item.command_id.clone(),
                    )
                })
                .collect::<Vec<_>>();

            let primary = labels
                .iter()
                .find(|(label, _, _)| label == "Primary")
                .unwrap();
            let control = labels
                .iter()
                .find(|(label, _, _)| label == "Control Key")
                .unwrap_or_else(|| {
                    panic!(
                        "Control Key missing; state={}, labels={labels:?}",
                        i.prin1_to_string(&state)
                    )
                });
            assert_eq!(primary.1, 0);
            assert_eq!(control.1, 1);
            let control_command = control
                .2
                .clone()
                .expect("modifier buttons retain a dispatchable command");

            let input_error = run_editor_with(
                ModifierBarFrontend {
                    events: VecDeque::from([
                        InputEvent::NativeMenuCommand(control_command),
                        InputEvent::Key('a' as i128),
                    ]),
                },
                &mut i,
            )
            .unwrap_err();
            assert_eq!(input_error.kind(), io::ErrorKind::UnexpectedEof);
            let seen = i.eval_str("qa-modifier-control-seen").unwrap();
            if !seen.truthy() {
                let state = [
                    "last-command",
                    "last-command-event",
                    "last-input-event",
                    "this-command",
                    "unread-command-events",
                ]
                .into_iter()
                .map(|name| {
                    let symbol = i.intern(name);
                    (name, i.prin1_to_string(&i.symbol_value(symbol)).to_unicode_lossy())
                })
                .collect::<Vec<_>>();
                panic!(
                    "clicking Control did not modify the next event: {state:?}"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
