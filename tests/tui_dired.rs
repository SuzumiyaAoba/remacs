//! Dired session via the scripted KeyIo loop: `M-x dired`, land on a
//! file line with C-s, `f`/`RET` visits it.  Repro for
//! `wrong-type-argument stringp t` seen when selecting a file.
use remacs::lisp::Interp;
use remacs::term::{Grid, KeyIo, render_grid};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;
use std::time::Duration;

struct ScriptIo {
    keys: VecDeque<i128>,
    echoes: Rc<RefCell<Vec<String>>>,
    grids: Rc<RefCell<Vec<Grid>>>,
}

impl KeyIo for ScriptIo {
    fn poll_key(&mut self, _d: Duration) -> io::Result<Option<i128>> {
        match self.keys.pop_front() {
            Some(k) => Ok(Some(k)),
            None => Err(io::Error::new(io::ErrorKind::TimedOut, "script done")),
        }
    }
    fn unread(&mut self, k: i128) {
        self.keys.push_front(k);
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let (w, h) = self.size();
        self.grids.borrow_mut().push(render_grid(i, w, h));
        Ok(())
    }
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        self.echoes.borrow_mut().push(text.to_string());
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

const C_X: i128 = 24;
const RET: i128 = b'\r' as i128;
const META: i128 = 0x0800_0000;
const C_S: i128 = 19;

fn keys(script: &mut Vec<i128>, s: &str) {
    script.extend(s.chars().map(|c| c as i128));
}

fn run_on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("test thread panicked");
}

#[test]
fn tui_dired_select_file() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        std::fs::create_dir_all("/tmp/rd").unwrap();
        std::fs::write("/tmp/rd/aaa.txt", "x\n").unwrap();

        let mut script = Vec::new();
        // M-x dired RET /tmp/rd RET — open dired on the fixture dir.
        script.push(META | 120);
        keys(&mut script, "dired");
        script.push(RET);
        keys(&mut script, "/tmp/rd");
        script.push(RET);
        // C-s aaa RET to land on the file line, then `f' to visit.
        script.push(C_S);
        keys(&mut script, "aaa.txt");
        script.push(RET); // exit isearch
        keys(&mut script, "f"); // dired-find-file on a real file
        script.extend([C_X, 'b' as i128]); // back to the dired buffer
        keys(&mut script, "rd");
        script.push(RET);
        // RET on the `.' line (point-min): exercises `.` handling.
        script.extend([C_X, b'b' as i128]);
        keys(&mut script, "rd");
        script.push(RET);
        script.push(META | '<' as i128);
        script.push(RET);
        // RET on the `..' line.
        script.push(b'n' as i128); // dired-next-line
        script.push(RET);
        // RET on the subdir line.
        script.push(C_S);
        keys(&mut script, "subdir");
        script.push(RET);
        script.push(RET);
        script.extend([C_X, 3]);
        let echoes = Rc::new(RefCell::new(Vec::new()));
        let grids = Rc::new(RefCell::new(Vec::new()));
        let io = ScriptIo { keys: script.into(), echoes: echoes.clone(), grids: grids.clone() };
        remacs::term::run_editor_with(io, &mut i).expect("editor loop");
        let bn = i.buffers.get(i.current_buffer).map(|b| b.borrow().name.clone());
        eprintln!("final buffer: {bn:?}");
        for e in echoes.borrow().iter() {
            eprintln!("  {e:?}");
        }
        for (gi, g) in grids.borrow().iter().enumerate() {
            eprintln!("--- grid {gi} ---");
            for r in &g.rows {
                eprintln!("  |{r}");
            }
        }
        assert!(!echoes.borrow().iter().any(|e| e.contains("wrong-type-argument")),
                "dired selection signaled: {:?}", echoes.borrow());
    })
}
