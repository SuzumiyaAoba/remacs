//! Subprocess and IPC object: `Value::Process` plus the Emacs process API.
//!
//! GNU Emacs supports real subprocesses, pipe processes, network
//! connections, serial ports, and listening servers. We implement the same
//! Lisp-visible model on top of `std::process`/`std::net`/libc: a `Proc`
//! records the command, status, buffer/filter/sentinel destinations, and
//! the underlying I/O handle. `accept-process-output` is the poll point —
//! it drains pending output into filters or process buffers and runs
//! sentinels on status transitions.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::Rc;

use super::builtins::S;
use super::error::Flow;
use super::eval::plist_get;
use super::obarray::sym;
use super::value::{Marker, MarkerRef, ProcessRef, Subr, Value};
use super::{EvalResult, Interp};

/// OS-level I/O behind a process object.
pub enum ProcIo {
    /// A real child subprocess connected through a pty (like GNU).
    Child {
        child: std::process::Child,
        /// Pty master: reads child stdout, writes child stdin.
        master_fd: i32,
        stderr: Option<std::process::ChildStderr>,
    },
    /// An Emacs-internal pipe. `read_fd` is polled for data a child
    /// writes into `child_wfd` (used as a `:stderr` destination);
    /// `sink_fd` swallows `process-send-string` output (GNU doesn't
    /// loop it back).
    Pipe {
        read_fd: i32,
        child_wfd: i32,
        sink_fd: i32,
    },
    /// A connected TCP stream.
    Net(std::net::TcpStream),
    /// A listening server socket.
    Listen(std::net::TcpListener),
    /// A serial port (file-backed).
    Serial(std::fs::File),
    /// No backend — GNU allows `make-process` without :command.
    None,
}

/// A live (or finished) process object.
pub struct Proc {
    pub name: String,
    /// `process-type`: "real" for children, "pipe", "network", "serial".
    pub kind: &'static str,
    /// Original :command list (or the contact description).
    pub command: Value,
    pub io: ProcIo,
    pub pid: i32,
    /// run / stop / exit / signal / open / listen / connect / failed.
    pub status: &'static str,
    pub exit_status: i32,
    /// Buffer receiving default-filter output.
    pub buffer: Option<usize>,
    /// Insertion marker into `buffer`.
    pub mark: Option<MarkerRef>,
    pub filter: Value,
    pub sentinel: Value,
    pub plist: Value,
    pub query_on_exit: bool,
    pub kill_without_query: bool,
    pub tty_name: Value,
    /// (decoding . encoding) coding system names.
    pub coding: (String, String),
    /// Where stderr goes: nil (discard), t (main output), buffer, or process.
    pub stderr_dest: Value,
    pub inherit_coding: bool,
    /// pipe / pty / serial / nil.
    pub connection_type: Value,
    /// `process-contact` value.
    pub contact: Value,
    /// Removed by `delete-process`.
    pub dead: bool,
    /// Final-state sentinel already delivered.
    pub reported: bool,
    /// :stop given at creation — start stopped.
    pub start_stopped: bool,
    /// Signal sent but not yet reflected in `status` (GNU applies
    /// status changes asynchronously via SIGCHLD notification).
    pub pending_status: Option<&'static str>,
}

impl Proc {
    pub fn alive(&self) -> bool {
        matches!(self.status, "run" | "stop" | "open" | "listen" | "connect")
    }
}

fn want_proc(i: &mut Interp, v: &Value) -> Result<ProcessRef, Flow> {
    match v {
        Value::Process(p) => Ok(p.clone()),
        // GNU's `get_process' accepts a process name string, and nil
        // means the current buffer's process.
        Value::Str(s) => {
            let name = s.borrow().clone();
            i.processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().name == name)
                .cloned()
                .ok_or_else(|| i.wrong_type_mut("processp", v))
        }
        Value::Nil => {
            let bid = i.current_buffer;
            match i
                .processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().buffer == Some(bid))
            {
                Some(p) => Ok(p.clone()),
                None => {
                    let name = i
                        .buffers
                        .get(bid)
                        .map(|b| b.borrow().name.clone())
                        .unwrap_or_default();
                    Err(i.error(format!("Buffer {} has no process", name)))
                }
            }
        }
        _ => Err(i.wrong_type_mut("processp", v)),
    }
}

fn symv(i: &mut Interp, s: &str) -> Value {
    Value::Sym(i.intern(s))
}

fn kw(i: &mut Interp, plist: &Value, name: &str) -> Value {
    let id = i.intern(name);
    plist_get(plist, id)
}

fn nonblock_fd(fd: i32) {
    unsafe {
        let fl = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, fl | libc::O_NONBLOCK);
    }
}

fn str_value(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.borrow().clone()),
        Value::Sym(_) => None,
        _ => None,
    }
}

/// Decode-process-output hook point: raw bytes → string.
/// We keep this UTF-8-lossy like GNU's default undecided/utf-8 handling.
fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// ---------- output / status engine ----------

/// Feed process output to its filter or buffer (no borrow held).
fn deliver_output(i: &mut Interp, pref: &ProcessRef, out: Vec<u8>, to_stderr_dest: bool) -> Result<(), Flow> {
    if out.is_empty() {
        return Ok(());
    }
    let (filter, buffer, mark, sdest) = {
        let p = pref.borrow();
        (
            p.filter.clone(),
            p.buffer,
            p.mark.clone(),
            p.stderr_dest.clone(),
        )
    };
    if to_stderr_dest {
        // Route to the configured stderr destination.
        match &sdest {
            Value::Nil => {}
            Value::Sym(id) if *id == sym::T => {
                deliver_output(i, pref, out, false)?;
            }
            v => {
                if let Value::Process(target) = v {
                    let t = target.clone();
                    deliver_output(i, &t, out, false)?;
                } else if let Some(bid) = i.buffer_id_of(v) {
                    insert_into_buffer(i, bid, None, &decode(&out));
                }
            }
        }
        return Ok(());
    }
    let text = decode(&out);
    if filter.truthy() {
        let args = Value::list(vec![Value::Process(pref.clone()), Value::string(text)]);
        i.call_function(&filter, &args, None)?;
        return Ok(());
    }
    if let Some(bid) = buffer {
        // Insert at the process mark, advancing it like GNU does.
        let pos = mark
            .as_ref()
            .map(|m| m.borrow().position)
            .unwrap_or_else(|| {
                i.buffers.get(bid).map(|b| b.borrow().text_len()).unwrap_or(0)
            });
        let n = text.chars().count();
        insert_into_buffer(i, bid, Some(pos), &text);
        if let Some(m) = &mark {
            m.borrow_mut().position = pos + n;
        }
    }
    Ok(())
}

fn insert_into_buffer(i: &mut Interp, bid: usize, pos: Option<usize>, text: &str) {
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        match pos {
            Some(p) => {
                let at = p.min(bb.text_len());
                bb.insert_at(at, text);
            }
            None => {
                let end = bb.text_len();
                bb.insert_at(end, text);
            }
        }
    }
}

fn event_string(status: &str, code: i32) -> String {
    match status {
        "exit" if code == 0 => "finished\n".to_string(),
        "exit" => format!("exited abnormally with code {code}\n"),
        "signal" => {
            let name = match code {
                libc::SIGHUP => "hangup",
                libc::SIGINT => "interrupt",
                libc::SIGQUIT => "quit",
                libc::SIGKILL => "killed",
                libc::SIGTERM => "terminated",
                libc::SIGSTOP => "stopped",
                libc::SIGTSTP => "stopped",
                libc::SIGPIPE => "broken pipe",
                _ => "signal",
            };
            format!("{name}: {code}\n")
        }
        "open" => "open\n".to_string(),
        _ => format!("{status}\n"),
    }
}

/// Run the sentinel for a status change, if any. With no sentinel GNU's
/// `internal-default-process-sentinel` echoes "Process NAME EVENT" to the
/// message stream (stderr in batch).
fn run_sentinel(i: &mut Interp, pref: &ProcessRef, status: &str, code: i32) -> Result<(), Flow> {
    let (sentinel, name) = {
        let pb = pref.borrow();
        (pb.sentinel.clone(), pb.name.clone())
    };
    let event = event_string(status, code);
    if sentinel.truthy() {
        let args = Value::list(vec![Value::Process(pref.clone()), Value::string(event)]);
        i.call_function(&sentinel, &args, None)?;
    } else if ![
        // GNU's internal-default-process-sentinel skips routine
        // status transitions; only abnormal/final events get a
        // "Process NAME EVENT" message.
        "open", "run", "listen", "connect", "closed", "deleted", "signal",
        "hangup", "killed", "terminated", "accept", "failed", "stop",
        "continued", "interrupt",
    ]
    .iter()
    .any(|p| event.starts_with(p))
    {
        eprintln!("Process {name} {event}");
    }
    Ok(())
}

/// Poll one process: drain output, observe exit. Returns true if anything
/// was consumed/changed.
fn poll_proc(i: &mut Interp, pref: &ProcessRef) -> Result<bool, Flow> {
    enum Ev {
        Out(Vec<u8>, bool),
        Exit(&'static str, i32),
        Accepted(std::net::TcpStream, std::net::SocketAddr),
    }
    let mut events = Vec::new();
    {
        let mut p = pref.borrow_mut();
        match &mut p.io {
            ProcIo::Child {
                child,
                master_fd,
                stderr,
            } => {
                let mut buf = [0u8; 8192];
                loop {
                    let n = unsafe {
                        libc::read(*master_fd, buf.as_mut_ptr() as *mut _, buf.len())
                    };
                    if n > 0 {
                        events.push(Ev::Out(buf[..n as usize].to_vec(), false));
                    } else {
                        break;
                    }
                }
                if let Some(s) = stderr.as_mut() {
                    loop {
                        match s.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => events.push(Ev::Out(buf[..n].to_vec(), true)),
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(_) => break,
                        }
                    }
                }
                // GNU notices process exit asynchronously; when output
                // arrived this poll the exit is picked up next round.
                match child.try_wait() {
                    Ok(Some(st)) if events.is_empty() => {
                        if let Some(sig) = std::os::unix::process::ExitStatusExt::signal(&st) {
                            p.status = "signal";
                            p.exit_status = sig;
                            events.push(Ev::Exit("signal", sig));
                        } else {
                            let code = st.code().unwrap_or(-1);
                            p.status = "exit";
                            p.exit_status = code;
                            events.push(Ev::Exit("exit", code));
                        }
                    }
                    _ => {}
                }
            }
            ProcIo::Pipe { read_fd, child_wfd, .. } => {
                let mut buf = [0u8; 8192];
                loop {
                    let n = unsafe { libc::read(*read_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
                    if n > 0 {
                        events.push(Ev::Out(buf[..n as usize].to_vec(), false));
                    } else {
                        // EOF once every writer (a child's stderr dup)
                        // is gone — GNU marks the pipe "closed".
                        if n == 0 && *child_wfd < 0 && p.status != "closed" {
                            p.status = "closed";
                            p.exit_status = 0;
                            events.push(Ev::Exit("finished", 0));
                        }
                        break;
                    }
                }
            }
            ProcIo::Net(s) => {
                let mut buf = [0u8; 8192];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) => {
                            if p.status != "exit" {
                                p.status = "exit";
                                p.exit_status = 0;
                                events.push(Ev::Exit("exit", 0));
                            }
                            break;
                        }
                        Ok(n) => events.push(Ev::Out(buf[..n].to_vec(), false)),
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
            }
            ProcIo::Listen(l) => loop {
                match l.accept() {
                    Ok((stream, addr)) => events.push(Ev::Accepted(stream, addr)),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            },
            ProcIo::Serial(f) => {
                let mut buf = [0u8; 8192];
                loop {
                    match f.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => events.push(Ev::Out(buf[..n].to_vec(), false)),
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
            }
            ProcIo::None => {}
        }
    }
    // accept-process-output returns t only when new output data
    // arrived — exits and accepts don't count in GNU.
    let did = events.iter().any(|e| matches!(e, Ev::Out(..)));
    for ev in events {
        match ev {
            Ev::Out(bytes, stderr) => deliver_output(i, pref, bytes, stderr)?,
            Ev::Exit(st, code) => {
                if !pref.borrow().reported {
                    pref.borrow_mut().reported = true;
                    run_sentinel(i, pref, st, code)?;
                }
            }
            Ev::Accepted(stream, addr) => {
                // A server accept produces a new network process named
                // "<host:port>" in GNU.
                let _ = stream.set_nonblocking(true);
                let _buffer = pref.borrow().buffer;
                // The accepted process gets its own buffer in GNU (nil
                // by default — data doesn't land in the server's).
                let buffer = None;
                let contact = Value::list(vec![
                    Value::string(addr.ip().to_string()),
                    Value::Int(addr.port() as i128),
                ]);

                let child_proc = Rc::new(RefCell::new(Proc {
                    // GNU names the accepted process "<host:port>".
                    name: format!("<{addr}>"),
                    kind: "network",
                    command: Value::Nil,
                    io: ProcIo::Net(stream),
                    pid: 0,
                    status: "open",
                    exit_status: 0,
                    buffer,
                    mark: None,
                    filter: Value::Nil,
                    sentinel: Value::Nil,
                    plist: Value::Nil,
                    query_on_exit: true,
                    kill_without_query: false,
                    tty_name: Value::Nil,
                    coding: ("utf-8-unix".into(), "utf-8-unix".into()),
                    stderr_dest: Value::Nil,
                    inherit_coding: false,
                    connection_type: Value::Nil,
                    contact,
                    dead: false,
                    reported: false,
                    start_stopped: false,
                    pending_status: None,
                }));
                i.processes.push(child_proc);
                // GNU re-registers the server in process_alist on accept,
                // so it can appear twice in `process-list`.
                i.processes.push(pref.clone());
                run_sentinel(i, pref, "open", 0)?;
            }
        }
    }
    Ok(did)
}

/// Poll all live processes once; returns true if anything happened.
fn poll_all(i: &mut Interp) -> Result<bool, Flow> {
    let procs = i.processes.clone();
    let mut did = false;
    for p in procs {
        if p.borrow().dead {
            continue;
        }
        did |= poll_proc(i, &p)?;
    }
    Ok(did)
}

// ---------- constructors ----------

fn base_proc(name: String, kind: &'static str, io: ProcIo) -> Proc {
    Proc {
        name,
        kind,
        command: Value::Nil,
        io,
        pid: 0,
        status: "run",
        exit_status: 0,
        buffer: None,
        mark: None,
        filter: Value::Nil,
        sentinel: Value::Nil,
        plist: Value::Nil,
        query_on_exit: true,
        kill_without_query: false,
        tty_name: Value::Nil,
        coding: ("utf-8-unix".into(), "utf-8-unix".into()),
        stderr_dest: Value::Nil,
        inherit_coding: false,
        connection_type: Value::Nil,
        contact: Value::Nil,
        dead: false,
        reported: false,
        start_stopped: false,
        pending_status: None,
    }
}

/// Shared :buffer/:filter/:sentinel/etc keyword handling.
fn finish_setup(
    i: &mut Interp,
    mut p: Proc,
    args: &Value,
    want_command: bool,
) -> Result<ProcessRef, Flow> {
    let b = kw(i, args, ":buffer");
    if b.truthy() {
        // GNU creates the named buffer when it doesn't exist.
        let bid = match i.buffer_id_of(&b) {
            Some(id) => id,
            None => match &b {
                Value::Str(s) => i.buffers.create(&s.borrow()),
                _ => return Err(i.error("No such buffer")),
            },
        };
        p.buffer = Some(bid);
        // Process mark sits at end of buffer.
        let end = i.buffers.get(bid).map(|r| r.borrow().text_len()).unwrap_or(0);
        p.mark = Some(Rc::new(RefCell::new(Marker {
            buffer: Some(bid),
            position: end,
            insertion_type: true,
        })));
    }
    p.filter = kw(i, args, ":filter");
    p.sentinel = kw(i, args, ":sentinel");
    p.plist = kw(i, args, ":plist");
    if kw(i, args, ":noquery").truthy() {
        p.query_on_exit = false;
    }
    if kw(i, args, ":stop").truthy() {
        p.start_stopped = true;
        p.pending_status = Some(p.status);
        p.status = "stop";
    }
    let ct = kw(i, args, ":connection-type");
    if ct.truthy() {
        p.connection_type = ct;
    }
    let sd = kw(i, args, ":stderr");
    if sd.truthy() {
        p.stderr_dest = sd;
    }
    let coding = kw(i, args, ":coding");
    let decode_side = match &coding {
        Value::Cons(c) => Some(c.borrow().car.clone()),
        v if v.truthy() => Some(v.clone()),
        _ => None,
    };
    if let Some(v) = decode_side.filter(|v| v.truthy()) {
        match &v {
            Value::Sym(s) => p.coding.0 = eol_normalize(&i.symbol_name(*s)),
            Value::Str(s) => p.coding.0 = eol_normalize(&s.borrow()),
            _ => {}
        }
    }
    let encode_side = match &coding {
        Value::Cons(c) => Some(c.borrow().cdr.clone()),
        _ => None,
    };
    if let Some(v) = encode_side.filter(|v| v.truthy()) {
        if let Value::Sym(s) = &v {
            p.coding.1 = eol_normalize(&i.symbol_name(*s));
        }
    }
    if want_command {
        p.command = kw(i, args, ":command");
    }
    Ok(Rc::new(RefCell::new(p)))
}

fn eol_normalize(name: &str) -> String {
    // GNU normalizes eol-variant coding systems to -unix on posix; names
    // that are already eol-invariant (latin-1, raw-text) pass through.
    match name {
        "utf-8" | "utf-8-auto" | "mule-utf-8" => "utf-8-unix".to_string(),
        "undecided" => "undecided-unix".to_string(),
        "prefer-utf-8" => "prefer-utf-8-unix".to_string(),
        s => s.to_string(),
    }
}

fn f_make_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let args = Value::list(a);
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if name_v.truthy() => i.symbol_name(*id),
        _ => return Err(i.error("Process name not given")),
    };
    // GNU quirk: a non-nil :stop on make-process signals
    // (wrong-type-argument null <value>); pipe processes accept :stop.
    let stop_v = kw(i, &args, ":stop");
    if stop_v.truthy() {
        let pred = i.intern("null");
        let wta = i.intern("wrong-type-argument");
        return Err(i.signal_data(wta, vec![Value::Sym(pred), stop_v]));
    }
    let command = kw(i, &args, ":command");
    let cmd: Vec<String> = match &command {
        Value::Cons(_) | Value::Nil => {
            let items = command.list_to_vec().map_err(|_| i.error("Bad :command"))?;
            let mut v = Vec::new();
            for it in items {
                match &it {
                    Value::Str(s) => v.push(s.borrow().clone()),
                    Value::Sym(id) => v.push(i.symbol_name(*id)),
                    _ => return Err(i.error("Bad :command")),
                }
            }
            v
        }
        _ => return Err(i.error("Bad :command")),
    };
    if cmd.is_empty() {
        // GNU allows make-process without :command — a "run"-status
        // process with no io backend.
        let p = base_proc(name.clone(), "real", ProcIo::None);
        let pr = finish_setup(i, p, &args, true)?;
        i.processes.push(pr.clone());
        return Ok(Value::Process(pr));
    }
    // GNU merges stderr into the pty by default; a non-nil :stderr
    // gives stderr its own pipe. As a destination GNU accepts a buffer
    // or a pipe process — anything else errors "Process is not a pipe
    // process".
    let stderr_dest = kw(i, &args, ":stderr");
    if let Value::Process(sp) = &stderr_dest {
        if !matches!(sp.borrow().io, ProcIo::Pipe { .. }) {
            return Err(i.error("Process is not a pipe process"));
        }
    }
    let split_stderr = stderr_dest.truthy();
    // GNU connects child processes through a pty: stdout/stdin share the
    // slave side, we keep the master.
    let (master_fd, slave_fd, tty_name) = unsafe {
        let mut master: libc::c_int = -1;
        let mut slave: libc::c_int = -1;
        if libc::openpty(&mut master, &mut slave, std::ptr::null_mut(),
                         std::ptr::null_mut(), std::ptr::null_mut()) != 0
        {
            return Err(i.error("Process not started: openpty failed"));
        }
        let name = libc::ttyname(slave);
        let name = if name.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned()
        };
        // GNU puts the subprocess pty into a raw-ish mode: no echo, no
        // canonical input, no output post-processing (so \n stays \n).
        let mut tio: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(slave, &mut tio) == 0 {
            tio.c_lflag &= !(libc::ECHO | libc::ICANON);
            tio.c_oflag &= !libc::OPOST;
            libc::tcsetattr(slave, libc::TCSANOW, &tio);
        }
        (master, slave, name)
    };
    // A pipe-process :stderr gets the pipe's write end dup'd straight
    // onto the child's stderr (GNU wires the fd, it doesn't route
    // bytes), then relinquishes the parent's copy.
    let mut stderr_pipe_wfd: Option<i32> = None;
    if let Value::Process(sp) = &stderr_dest {
        let mut spb = sp.borrow_mut();
        if let ProcIo::Pipe { child_wfd, .. } = &mut spb.io {
            if *child_wfd >= 0 {
                stderr_pipe_wfd = Some(std::mem::replace(child_wfd, -1));
            }
        }
    }
    let mut command = std::process::Command::new(&cmd[0]);
    command.args(&cmd[1..]);
    {
        use std::os::unix::io::FromRawFd;
        unsafe {
            let in_f = std::fs::File::from_raw_fd(libc::dup(slave_fd));
            let out_f = std::fs::File::from_raw_fd(libc::dup(slave_fd));
            command.stdin(std::process::Stdio::from(in_f));
            command.stdout(std::process::Stdio::from(out_f));
            if let Some(wfd) = stderr_pipe_wfd {
                let err_f = std::fs::File::from_raw_fd(wfd);
                command.stderr(std::process::Stdio::from(err_f));
            } else if split_stderr {
                command.stderr(std::process::Stdio::piped());
            } else {
                let err_f = std::fs::File::from_raw_fd(libc::dup(slave_fd));
                command.stderr(std::process::Stdio::from(err_f));
            }
        }
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            unsafe {
                libc::close(master_fd);
                libc::close(slave_fd);
            }
            return Err(i.signal_data(
                sym::FILE_MISSING,
                vec![
                    Value::string("Searching for program"),
                    Value::string("No such file or directory"),
                    Value::string(cmd[0].clone()),
                ],
            ));
        }
        Err(e) => {
            unsafe {
                libc::close(master_fd);
                libc::close(slave_fd);
            }
            return Err(i.error(format!("Process not started: {e}")));
        }
    };
    unsafe { libc::close(slave_fd) };
    nonblock_fd(master_fd);
    let stderr = child.stderr.take();
    {
        use std::os::unix::io::AsRawFd;
        if let Some(s) = &stderr {
            nonblock_fd(s.as_raw_fd());
        }
    }
    let pid = child.id() as i32;
    let mut p = base_proc(name, "real", ProcIo::Child {
        child,
        master_fd,
        stderr,
    });
    p.tty_name = Value::string(tty_name);
    p.pid = pid;
    let pref = finish_setup(i, p, &args, true)?;
    i.processes.push(pref.clone());
    if pref.borrow().start_stopped {
        unsafe { libc::kill(pid, libc::SIGSTOP) };
    }
    Ok(Value::Process(pref))
}

fn f_make_pipe_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let args = Value::list(a);
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if name_v.truthy() => i.symbol_name(*id),
        _ => return Err(i.error("Pipe process name not given")),
    };
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(i.error("Cannot create pipe"));
    }
    // A second pipe carries data sent via `process-send-string`; its
    // read end is never polled (GNU doesn't loop it back either).
    let mut wfds = [0i32; 2];
    if unsafe { libc::pipe(wfds.as_mut_ptr()) } != 0 {
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        return Err(i.error("Cannot create pipe"));
    }
    nonblock_fd(fds[0]);
    nonblock_fd(fds[1]);
    nonblock_fd(wfds[1]);
    let mut p = base_proc(name, "pipe", ProcIo::Pipe {
        read_fd: fds[0],
        child_wfd: fds[1],
        sink_fd: wfds[1],
    });
    p.status = "open";
    // GNU's `process-contact' returns the creation plist.
    p.contact = args.clone();
    let pref = finish_setup(i, p, &args, false)?;
    i.processes.push(pref.clone());
    Ok(Value::Process(pref))
}

fn service_port(i: &mut Interp, v: &Value) -> Result<u16, Flow> {
    match v {
        Value::Int(n) => Ok((*n).clamp(0, 65535) as u16),
        Value::Str(s) => {
            let t = s.borrow().clone();
            if let Ok(n) = t.parse::<u16>() {
                return Ok(n);
            }
            let port = match t.as_str() {
                "http" => 80,
                "https" => 443,
                "ftp" => 21,
                "ssh" => 22,
                "telnet" => 23,
                "smtp" => 25,
                "domain" => 53,
                "imap" => 143,
                "imaps" => 993,
                "pop3" => 110,
                "pop3s" => 995,
                _ => return Err(i.error(format!("Unknown service: {t}"))),
            };
            Ok(port)
        }
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

fn f_make_network_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let args = Value::list(a);
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if name_v.truthy() => i.symbol_name(*id),
        _ => return Err(i.error("Network process name not given")),
    };
    let service = kw(i, &args, ":service");
    let server = kw(i, &args, ":server").truthy();
    let host_v = kw(i, &args, ":host");
    let host = match &host_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if host_v.truthy() => i.symbol_name(*id),
        _ => "127.0.0.1".to_string(),
    };
    // GNU's `process-contact' returns the creation plist with :service
    // resolved, plus :local (and :remote for clients) address vectors.
    let contact_plist = |i: &mut Interp, port: u16, extra: Vec<Value>| -> Value {
        let mut items: Vec<Value> = Vec::new();
        let mut skip = false;
        for v in args.list_to_vec().unwrap_or_default() {
            if skip {
                skip = false;
                items.push(Value::Int(port as i128));
                continue;
            }
            if matches!(&v, Value::Sym(s) if i.symbol_name(*s) == ":service") {
                skip = true;
            }
            items.push(v);
        }
        items.extend(extra);
        Value::list(items)
    };
    if server {
        let port = service_port(i, &service)?;
        let bind_host = if kw(i, &args, ":family").truthy() { "0.0.0.0".to_string() } else { host.clone() };
        let listener = std::net::TcpListener::bind((bind_host.as_str(), port))
            .map_err(|e| i.error(format!("make-network-process: {e}")))?;
        let _ = listener.set_nonblocking(true);
        let local_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let mut p = base_proc(name, "network", ProcIo::Listen(listener));
        p.status = "listen";
        let local_kw = symv(i, ":local");
        let local_v = addr_vec("127.0.0.1", local_port);
        p.contact = contact_plist(i, local_port, vec![local_kw, local_v]);
        let pref = finish_setup(i, p, &args, false)?;
        i.processes.push(pref.clone());
        return Ok(Value::Process(pref));
    }
    let port = service_port(i, &service)?;
    let stream = match std::net::TcpStream::connect((host.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            // GNU: (file-error "make client process failed" <errno> <plist>)
            let fe = i.intern("file-error");
            let mut data = vec![
                Value::string("make client process failed"),
                Value::string(e.to_string()),
            ];
            data.extend(args.list_to_vec().unwrap_or_default());
            return Err(i.signal_data(fe, data));
        }
    };
        let _ = stream.set_nonblocking(true);
    let local_port = stream.local_addr().map(|a| a.port()).unwrap_or(0);
    let mut p = base_proc(name, "network", ProcIo::Net(stream));
    p.status = "open";
    let remote_kw = symv(i, ":remote");
    let local_kw = symv(i, ":local");
    p.contact = contact_plist(
        i,
        port,
        vec![
            remote_kw,
            addr_vec(&host, port),
            local_kw,
            addr_vec("127.0.0.1", local_port),
        ],
    );
    let pref = finish_setup(i, p, &args, false)?;
    i.processes.push(pref.clone());
    Ok(Value::Process(pref))
}

/// `[a b c d port]` address vector like GNU's contact format.
fn addr_vec(host: &str, port: u16) -> Value {
    let mut octets: Vec<Value> = host
        .split('.')
        .filter_map(|o| o.parse::<u8>().ok())
        .map(|o| Value::Int(o as i128))
        .collect();
    while octets.len() < 4 {
        octets.push(Value::Int(0));
    }
    octets.push(Value::Int(port as i128));
    Value::Vec(Rc::new(RefCell::new(octets)))
}

fn f_make_serial_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let args = Value::list(a);
    // GNU returns nil for (make-serial-process) with no arguments.
    if matches!(args, Value::Nil) {
        return Ok(Value::Nil);
    }
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if name_v.truthy() => i.symbol_name(*id),
        _ => "serial".to_string(),
    };
    let port_v = kw(i, &args, ":port");
    if !port_v.truthy() {
        return Err(i.error("No port specified"));
    }
    let path = str_value(&port_v).ok_or_else(|| i.error("Serial port not given"))?;
    // GNU requires :speed.
    if !kw(i, &args, ":speed").truthy() {
        return Err(i.error(":speed not specified"));
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| {
            // GNU: (file-missing "Opening serial port" <errno> <path>)
            let fm = i.intern("file-missing");
            i.signal_data(
                fm,
                vec![
                    Value::string("Opening serial port"),
                    Value::string(e.to_string()),
                    Value::string(path.clone()),
                ],
            )
        })?;
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        // GNU runs tcgetattr on the port and signals
        // (file-error "Failed tcgetattr" <errno>) when it fails.
        let mut tio: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut tio) } != 0 {
            let e = std::io::Error::last_os_error();
            let fe = i.intern("file-error");
            return Err(i.signal_data(
                fe,
                vec![
                    Value::string("Failed tcgetattr"),
                    Value::string(e.to_string()),
                ],
            ));
        }
        nonblock_fd(fd);
    }
    let mut p = base_proc(name, "serial", ProcIo::Serial(file));
    // GNU serial status is "open"; contact is the creation plist, with
    // (PORT SPEED) derived for the no-key form.
    p.status = "open";
    p.contact = args.clone();
    p.connection_type = symv(i, "serial");
    let pref = finish_setup(i, p, &args, false)?;
    if let Value::Int(speed) = kw(i, &args, ":speed") {
        serial_set_speed(&pref, speed as u32);
    }
    i.processes.push(pref.clone());
    Ok(Value::Process(pref))
}

fn serial_set_speed(pref: &ProcessRef, speed: u32) {
    let mut p = pref.borrow_mut();
    if let ProcIo::Serial(f) = &mut p.io {
        use std::os::unix::io::AsRawFd;
        unsafe {
            let mut tio: libc::termios = std::mem::zeroed();
            let fd = f.as_raw_fd();
            if libc::tcgetattr(fd, &mut tio) == 0 {
                let _ = libc::cfsetspeed(&mut tio, speed as libc::speed_t);
                libc::tcsetattr(fd, libc::TCSANOW, &tio);
            }
        }
    }
}

fn f_serial_process_configure(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let args = Value::list(a);
    let pv = kw(i, &args, ":process");
    let pref = want_proc(i, &pv)?;
    if let Value::Int(speed) = kw(i, &args, ":speed") {
        serial_set_speed(&pref, speed as u32);
    }
    Ok(Value::Nil)
}

fn f_start_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (name buffer program &rest args)
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) => i.symbol_name(*id),
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    };
    let mut plist_items = vec![
        symv(i, ":name"),
        Value::string(name),
        symv(i, ":buffer"),
        a[1].clone(),
    ];
    if a.len() >= 3 && a[2].truthy() {
        let mut cmd = vec![a[2].clone()];
        cmd.extend(a[3..].iter().cloned());
        plist_items.push(symv(i, ":command"));
        plist_items.push(Value::list(cmd));
    }
    f_make_process(i, plist_items)
}

// ---------- predicates / accessors ----------

fn f_processp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Process(_))))
}

fn f_get_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Process(_) => Ok(a[0].clone()),
        Value::Str(s) => {
            let name = s.borrow().clone();
            for p in &i.processes {
                if !p.borrow().dead && p.borrow().name == name {
                    return Ok(Value::Process(p.clone()));
                }
            }
            Ok(Value::Nil)
        }
        // GNU's `get_process' resolves nil to the current buffer's
        // process, signalling "Buffer %s has no process" when none.
        Value::Nil => want_proc(i, &a[0]).map(|p| Value::Process(p)),
        _ => Ok(Value::Nil),
    }
}

fn f_process_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU's Vprocess_alist pushes new processes to the front; deleted
    // processes keep whatever alist cells weren't removed, so don't
    // filter on `dead`.
    let v: Vec<Value> = i
        .processes
        .iter()
        .rev()
        .map(|p| Value::Process(p.clone()))
        .collect();
    Ok(Value::list(v))
}

fn f_list_processes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU displays a "*Process List*" buffer and returns nil; when called
    // with QUERY-ONLY it filters. Keep the display minimal: create the
    // buffer and return nil like GNU.
    let _ = a;
    let bid = i.buffers.create("*Process List*");
    let mut lines = String::from("Proc         Status   Buffer         Tty         Command\n----         ------   ------         ---         -------\n");
    let procs = i.processes.clone();
    for p in procs {
        let pb = p.borrow();
        if pb.dead {
            continue;
        }
        let bufname = pb
            .buffer
            .and_then(|b| i.buffer_name(b))
            .unwrap_or_else(|| "(none)".into());
        lines.push_str(&format!(
            "{:<13}{:<9}{:<15}{:<12}{}\n",
            pb.name, pb.status, bufname, "-", "-"
        ));
    }
    if let Some(b) = i.buffers.get(bid) {
        let mut bb = b.borrow_mut();
        bb.insert_at(0, &lines);
        bb.point = 0;
    }
    Ok(Value::Nil)
}

fn f_process_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::string(p.borrow().name.clone()))
}

fn f_process_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let bid = p.borrow().buffer;
    Ok(bid.and_then(|b| i.buffer_value(b)).unwrap_or(Value::Nil))
}

fn f_set_process_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let bid = if a[1].is_nil() {
        None
    } else {
        Some(
            i.buffer_id_of(&a[1])
                .ok_or_else(|| i.error("No such buffer"))?,
        )
    };
    let mut pb = p.borrow_mut();
    pb.buffer = bid;
    let end = bid.and_then(|b| i.buffers.get(b)).map(|r| r.borrow().text_len());
    if let (Some(b), Some(e)) = (bid, end) {
        match &pb.mark {
            Some(m) => m.borrow_mut().position = e,
            None => {
                pb.mark = Some(Rc::new(RefCell::new(Marker {
                    buffer: Some(b),
                    position: e,
                    insertion_type: true,
                })))
            }
        }
    }
    Ok(Value::Nil)
}

fn f_process_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().command.clone())
}

fn f_process_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let k = p.borrow().kind;
    Ok(symv(i, k))
}

fn f_process_id(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::Int(p.borrow().pid as i128))
}

fn f_process_status(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU reads the cached status — updated only by status notification
    // during waits (accept-process-output et al), not by this query.
    let p = want_proc(i, &a[0])?;
    Ok(symv(i, p.borrow().status))
}

fn f_process_exit_status(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::Int(p.borrow().exit_status as i128))
}

fn f_process_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    // GNU lazily creates the mark in the current buffer when the
    // process has none, and adopts that buffer as the process buffer.
    let bid = {
        let pb = p.borrow();
        match pb.buffer {
            Some(b) => Some(b),
            None => {
                drop(pb);
                let bid = i.current_buffer;
                p.borrow_mut().buffer = Some(bid);
                Some(bid)
            }
        }
    };
    let mut pb = p.borrow_mut();
    if let Some(m) = &pb.mark {
        return Ok(Value::Marker(m.clone()));
    }
    if let Some(bid) = bid {
        let m = Rc::new(RefCell::new(Marker {
            buffer: Some(bid),
            position: 0,
            insertion_type: true,
        }));
        pb.mark = Some(m.clone());
        return Ok(Value::Marker(m));
    }
    Ok(Value::Nil)
}

fn f_process_contact(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (c, kind) = {
        let pb = p.borrow();
        (pb.contact.clone(), pb.kind)
    };
    if kind == "real" {
        // GNU returns t for ordinary child processes.
        return Ok(Value::t());
    }
    let key = a.get(1).cloned().unwrap_or(Value::Nil);
    match &key {
        Value::Nil => match kind {
            // (host service) list — :local for servers, :remote for clients.
            "network" => {
                let host = crate::lisp::eval::plist_get(&c, i.intern(":host"));
                let service = crate::lisp::eval::plist_get(&c, i.intern(":service"));
                Ok(Value::list(vec![host, service]))
            }
            // serial: (PORT SPEED)
            "serial" => {
                let port = crate::lisp::eval::plist_get(&c, i.intern(":port"));
                let speed = crate::lisp::eval::plist_get(&c, i.intern(":speed"));
                Ok(Value::list(vec![port, speed]))
            }
            _ => Ok(Value::Nil),
        },
        Value::Sym(id) if i.symbol_name(*id) == "t" => Ok(c),
        Value::Sym(id) => {
            let v = crate::lisp::eval::plist_get(&c, *id);
            Ok(v)
        }
        _ => Ok(c),
    }
}

fn f_process_plist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().plist.clone())
}

fn f_set_process_plist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().plist = a[1].clone();
    Ok(a[1].clone())
}

fn f_process_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let prop = match &a[1] {
        Value::Sym(id) => *id,
        _ => return Err(i.wrong_type_mut("symbolp", &a[1])),
    };
    Ok(plist_get(&p.borrow().plist, prop))
}

fn f_process_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let prop = match &a[1] {
        Value::Sym(id) => *id,
        _ => return Err(i.wrong_type_mut("symbolp", &a[1])),
    };
    let mut pb = p.borrow_mut();
    pb.plist = super::eval::plist_put(&pb.plist, prop, a[2].clone());
    // GNU returns the updated plist.
    Ok(pb.plist.clone())
}

fn f_process_query_on_exit_flag(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::from_bool(p.borrow().query_on_exit))
}

fn f_set_process_query_on_exit_flag(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().query_on_exit = a[1].truthy();
    Ok(a[1].clone())
}

fn f_process_filter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let f = p.borrow().filter.clone();
    if f.truthy() {
        Ok(f)
    } else {
        Ok(symv(i, "internal-default-process-filter"))
    }
}

fn f_set_process_filter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().filter = a[1].clone();
    Ok(a[1].clone())
}

fn f_process_sentinel(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().sentinel.clone())
}

fn f_set_process_sentinel(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().sentinel = a[1].clone();
    Ok(a[1].clone())
}

fn f_process_tty_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().tty_name.clone())
}

fn f_process_coding_system(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (d, e) = p.borrow().coding.clone();
    Ok(Value::cons(symv(i, &d), symv(i, &e)))
}

fn f_set_process_coding_system(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let mut pb = p.borrow_mut();
    // GNU stores the decode side verbatim; the encode side gets the
    // canonical name with the -unix eol variant.
    if let Value::Sym(s) = &a[1] {
        pb.coding.0 = i.symbol_name(*s);
    }
    if a.len() > 2 {
        if let Value::Sym(s) = &a[2] {
            pb.coding.1 = coding_canonical_unix(&i.symbol_name(*s));
        }
    }
    Ok(Value::Nil)
}

/// Canonical name + `-unix` suffix for the encode side of
/// `set-process-coding-system` (latin-1 → iso-latin-1-unix,
/// binary → raw-text-unix, utf-8 → utf-8-unix).
fn coding_canonical_unix(name: &str) -> String {
    let canon = match name.strip_prefix("latin-") {
        Some(n) => format!("iso-latin-{n}"),
        None => match name {
            "binary" => "raw-text".to_string(),
            "sjis" => "shift-jis".to_string(),
            s => s.to_string(),
        },
    };
    if canon.ends_with("-unix") || canon.ends_with("-dos") || canon.ends_with("-mac") {
        canon
    } else {
        format!("{canon}-unix")
    }
}

fn f_process_inherit_coding_system_flag(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::from_bool(p.borrow().inherit_coding))
}

fn f_set_process_inherit_coding_system_flag(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().inherit_coding = a[1].truthy();
    Ok(a[1].clone())
}

fn f_set_process_window_size(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_process_datagram_address(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_proc(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_set_process_datagram_address(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_proc(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_get_buffer_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bid = match &a[0] {
        Value::Nil => Some(i.current_buffer),
        v => i.buffer_id_of(v),
    };
    let Some(bid) = bid else { return Ok(Value::Nil) };
    for p in &i.processes {
        let pb = p.borrow();
        if !pb.dead && pb.buffer == Some(bid) {
            return Ok(Value::Process(p.clone()));
        }
    }
    Ok(Value::Nil)
}

// ---------- I/O ----------

fn f_accept_process_output(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (process &optional seconds millisec just-this-one)
    let target = match a.first().filter(|v| v.truthy()) {
        Some(v) => Some(want_proc(i, v)?),
        None => None,
    };
    // GNU requires SECONDS to be an integer (or nil).
    let secs = match a.get(1) {
        None | Some(Value::Nil) => 0.0,
        Some(Value::Int(n)) => *n as f64,
        Some(v) => {
            let wta = i.intern("wrong-type-argument");
            let fxp = i.intern("fixnump");
            return Err(i.signal_data(wta, vec![Value::Sym(fxp), v.clone()]));
        }
    };
    let millis = match a.get(2) {
        None | Some(Value::Nil) => 0.0,
        Some(Value::Int(n)) => *n as f64,
        Some(v) => {
            let wta = i.intern("wrong-type-argument");
            let fxp = i.intern("fixnump");
            return Err(i.signal_data(wta, vec![Value::Sym(fxp), v.clone()]));
        }
    };
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs_f64((secs + millis / 1000.0).max(0.0));
    // Always poll at least once.
    let mut got = match &target {
        Some(p) => poll_proc(i, p)?,
        None => poll_all(i)?,
    };
    while !got && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
        got = match &target {
            Some(p) => poll_proc(i, p)?,
            None => poll_all(i)?,
        };
        // A dead target can't produce more output — GNU returns early.
        if let Some(p) = &target {
            if !p.borrow().alive() {
                break;
            }
        }
    }
    Ok(Value::from_bool(got))
}

fn f_process_send_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let text = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &a[1])),
    };
    proc_write(i, &p, text.as_bytes())
}

fn proc_write(i: &mut Interp, p: &ProcessRef, bytes: &[u8]) -> EvalResult {
    let mut pb = p.borrow_mut();
    if !pb.alive() {
        return Err(i.error("Process is not running"));
    }
    let r = match &mut pb.io {
        ProcIo::Child { master_fd, .. } => {
            let n = unsafe { libc::write(*master_fd, bytes.as_ptr() as *const _, bytes.len()) };
            if n < 0 { Err(()) } else { Ok(()) }
        }
        ProcIo::Pipe { sink_fd, .. } => {
            let n = unsafe { libc::write(*sink_fd, bytes.as_ptr() as *const _, bytes.len()) };
            if n < 0 { Err(()) } else { Ok(()) }
        }
        ProcIo::Net(s) => s.write_all(bytes).map_err(|_| ()),
        ProcIo::Serial(f) => f.write_all(bytes).map_err(|_| ()),
        ProcIo::Listen(_) | ProcIo::None => Err(()),
    };
    match r {
        Ok(()) => Ok(Value::Nil),
        Err(()) => Err(i.error("Writing to process: broken pipe")),
    }
}

fn f_process_send_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (start, end) = (a[1].int(), a[2].int());
    let (Some(s), Some(e)) = (start, end) else {
        return Err(i.wrong_type_mut("integerp", &a[1]));
    };
    let bid = i.current_buffer;
    let text = i
        .buffers
        .get(bid)
        .map(|b| {
            let bb = b.borrow();
            let s = ((s - 1).max(0) as usize).min(bb.text_len());
            let e = ((e - 1).max(0) as usize).min(bb.text_len());
            let (lo, hi) = if s <= e { (s, e) } else { (e, s) };
            bb.text.text().chars().skip(lo).take(hi - lo).collect::<String>()
        })
        .unwrap_or_default();
    proc_write(i, &p, text.as_bytes())
}

fn f_process_send_eof(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    {
        let mut pb = p.borrow_mut();
        match &mut pb.io {
            // On a pty GNU writes the VEOF character.
            ProcIo::Child { master_fd, .. } => unsafe {
                libc::write(*master_fd, b"\x04".as_ptr() as *const _, 1);
            },
            ProcIo::Pipe { sink_fd, .. } => unsafe {
                libc::close(*sink_fd);
                *sink_fd = -1;
            },
            _ => {}
        }
    }
    // GNU returns the process.
    Ok(Value::Process(p))
}

// ---------- control ----------

fn signal_number(i: &mut Interp, v: &Value) -> Result<i32, Flow> {
    match v {
        Value::Int(n) => Ok((*n).clamp(0, 64) as i32),
        Value::Sym(id) => {
            let name = i.symbol_name(*id);
            let up = name.to_uppercase();
            let bare = up.strip_prefix("SIG").unwrap_or(&up);
            let num = match bare {
                "HUP" => libc::SIGHUP,
                "INT" => libc::SIGINT,
                "QUIT" => libc::SIGQUIT,
                "ILL" => libc::SIGILL,
                "TRAP" => libc::SIGTRAP,
                "ABRT" => libc::SIGABRT,
                "EMT" => 7,
                "FPE" => libc::SIGFPE,
                "KILL" => libc::SIGKILL,
                "BUS" => libc::SIGBUS,
                "SEGV" => libc::SIGSEGV,
                "SYS" => libc::SIGSYS,
                "PIPE" => libc::SIGPIPE,
                "ALRM" => libc::SIGALRM,
                "TERM" => libc::SIGTERM,
                "URG" => libc::SIGURG,
                "STOP" => libc::SIGSTOP,
                "TSTP" => libc::SIGTSTP,
                "CONT" => libc::SIGCONT,
                "CHLD" | "CLD" => libc::SIGCHLD,
                "TTIN" => libc::SIGTTIN,
                "TTOU" => libc::SIGTTOU,
                "IO" => libc::SIGIO,
                "XCPU" => libc::SIGXCPU,
                "XFSZ" => libc::SIGXFSZ,
                "VTALRM" => libc::SIGVTALRM,
                "PROF" => libc::SIGPROF,
                "WINCH" => libc::SIGWINCH,
                "USR1" => libc::SIGUSR1,
                "USR2" => libc::SIGUSR2,
                _ => return Err(i.error(format!("Undefined signal: {name}"))),
            };
            Ok(num)
        }
        _ => Err(i.wrong_type_mut("integerp", v)),
    }
}

fn f_signal_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = match &a[0] {
        Value::Int(n) => *n as i32,
        // GNU accepts a process object or a process name.
        _ => want_proc(i, &a[0])?.borrow().pid,
    };
    let sig = signal_number(i, &a[1])?;
    let r = unsafe { libc::kill(pid, sig) };
    Ok(Value::Int(r as i128))
}

fn signal_proc_arg(i: &mut Interp, a: &[Value], sig: i32) -> Result<ProcessRef, Flow> {
    let target = match a.first().filter(|v| v.truthy()) {
        Some(v) => want_proc(i, v)?,
        None => i
            .processes
            .iter()
            .find(|p| !p.borrow().dead && p.borrow().buffer == Some(i.current_buffer))
            .cloned()
            .ok_or_else(|| i.error("Current buffer has no process"))?,
    };
    {
        let pb = target.borrow();
        // GNU: "Process %s is not active" on a dead/exited process.
        if pb.dead || matches!(pb.status, "exit" | "signal" | "closed" | "failed") {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not active")));
        }
        // GNU's signal functions only apply to real subprocesses; stop
        // and continue are allowed on all live processes.
        let is_child = matches!(pb.io, ProcIo::Child { .. });
        if !is_child && !matches!(sig, libc::SIGTSTP | libc::SIGCONT) {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not a subprocess")));
        }
    }
    let pid = target.borrow().pid;
    if pid > 0 {
        unsafe { libc::kill(pid, sig) };
    }
    Ok(target)
}

fn f_interrupt_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's process-control functions return the process object.
    Ok(Value::Process(signal_proc_arg(i, &a, libc::SIGINT)?))
}
fn f_kill_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::Process(signal_proc_arg(i, &a, libc::SIGKILL)?))
}
fn f_quit_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::Process(signal_proc_arg(i, &a, libc::SIGQUIT)?))
}
fn f_stop_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = signal_proc_arg(i, &a, libc::SIGTSTP)?;
    // GNU's `stop` status is flow-control for non-subprocesses; a real
    // subprocess just gets SIGTSTP and stays "run".
    let mut pb = p.borrow_mut();
    if !matches!(pb.io, ProcIo::Child { .. }) {
        pb.pending_status = Some(pb.status);
        pb.status = "stop";
    }
    Ok(Value::Process(p.clone()))
}
fn f_continue_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = signal_proc_arg(i, &a, libc::SIGCONT)?;
    let mut pb = p.borrow_mut();
    if !matches!(pb.io, ProcIo::Child { .. }) {
        if let Some(prev) = pb.pending_status.take() {
            pb.status = prev;
        }
    }
    Ok(Value::Process(p.clone()))
}

fn f_delete_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let ev;
    {
        let mut pb = p.borrow_mut();
        pb.dead = true;
        ev = match &mut pb.io {
            ProcIo::Child { child, .. } => {
                let _ = child.kill();
                pb.status = "signal";
                pb.exit_status = libc::SIGKILL;
                ("signal", libc::SIGKILL)
            }
            ProcIo::Pipe { read_fd, child_wfd, sink_fd } => {
                unsafe {
                    libc::close(*read_fd);
                    if *child_wfd >= 0 {
                        libc::close(*child_wfd);
                    }
                    if *sink_fd >= 0 {
                        libc::close(*sink_fd);
                    }
                }
                pb.status = "closed";
                ("deleted", 0)
            }
            // GNU marks deleted network/serial processes "closed".
            ProcIo::Net(_) | ProcIo::Serial(_) | ProcIo::Listen(_) | ProcIo::None => {
                pb.status = "closed";
                ("deleted", 0)
            }
        };
    }
    // GNU removes only the oldest alist cell for the process.
    if let Some(pos) = i.processes.iter().position(|q| Rc::ptr_eq(q, &p)) {
        i.processes.remove(pos);
    }
    run_sentinel(i, &p, ev.0, ev.1)?;
    Ok(Value::Nil)
}

fn f_process_running_child_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU takes a PROCESS (default: the current buffer's process) and
    // returns non-nil when its pty's foreground process group differs
    // from the direct child — i.e. the child spawned grandchildren.
    let p = match a.first().unwrap_or(&Value::Nil) {
        Value::Nil => {
            let bid = i.current_buffer;
            match i
                .processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().buffer == Some(bid))
            {
                Some(p) => p.clone(),
                None => {
                    let name = i
                        .buffers
                        .get(bid)
                        .map(|b| b.borrow().name.clone())
                        .unwrap_or_default();
                    return Err(i.error(format!("Buffer {} has no process", name)));
                }
            }
        }
        v => want_proc(i, v)?,
    };
    let pb = p.borrow();
    if let ProcIo::Child { child, master_fd, .. } = &pb.io {
        if pb.status == "run" || pb.status == "stop" {
            let fg = unsafe { libc::tcgetpgrp(*master_fd) };
            if fg > 0 && fg != child.id() as i32 {
                return Ok(Value::Int(fg as i128));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_internal_default_process_filter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let text = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &a[1])),
    };
    let (buffer, mark) = {
        let pb = p.borrow();
        (pb.buffer, pb.mark.clone())
    };
    if let Some(bid) = buffer {
        let pos = mark.as_ref().map(|m| m.borrow().position);
        let n = text.chars().count();
        insert_into_buffer(i, bid, pos, &text);
        if let (Some(m), Some(p0)) = (&mark, pos) {
            m.borrow_mut().position = p0 + n;
        }
    }
    Ok(Value::Nil)
}

// ---------- synchronous helpers ----------

fn run_lines(i: &mut Interp, prog: &str, args: &[String], ignore_status: bool) -> EvalResult {
    let out = std::process::Command::new(prog)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) => {
            if !o.status.success() && !ignore_status {
                return Err(i.error(format!("{prog} exited with status {}", o.status.code().unwrap_or(-1))));
            }
            let text = decode(&o.stdout);
            let mut lines: Vec<&str> = text.split('\n').collect();
            if matches!(lines.last(), Some(l) if l.is_empty()) {
                lines.pop();
            }
            Ok(Value::list(
                lines.into_iter().map(Value::string).collect(),
            ))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Err(i.signal_data(
            sym::FILE_MISSING,
            vec![
                Value::string("Searching for program"),
                Value::string("No such file or directory"),
                Value::string(prog),
            ],
        )),
        Err(e) => Err(i.error(format!("{prog}: {e}"))),
    }
}

fn process_lines(i: &mut Interp, a: &[Value], ignore: bool) -> EvalResult {
    let prog = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    };
    let mut rest = Vec::new();
    for v in &a[1..] {
        match v {
            Value::Str(s) => rest.push(s.borrow().clone()),
            Value::Int(n) => rest.push(n.to_string()),
            _ => return Err(i.wrong_type_mut("stringp", v)),
        }
    }
    run_lines(i, &prog, &rest, ignore)
}

fn f_process_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    process_lines(i, &a, false)
}
fn f_process_lines_ignore_status(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    process_lines(i, &a, true)
}

// ---------- system enumeration ----------

fn f_list_system_processes(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let mut pids = Vec::new();
    for pid in 1..=99999i32 {
        let r = unsafe { libc::kill(pid, 0) };
        if r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
            pids.push(Value::Int(pid as i128));
        }
    }
    Ok(Value::list(pids))
}

fn f_process_attributes(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pid = match &a[0] {
        Value::Process(p) => p.borrow().pid,
        Value::Int(n) => *n as i32,
        _ => return Err(i.wrong_type_mut("integerp", &a[0])),
    };
    // GNU's alist via ps(1): fixed key set in fixed order, time fields
    // as (hi lo usec psec) lists.
    // args goes last so all space-free fields split cleanly. macOS ps
    // has no egid/thcount/majflt keywords — gid/uid are effective ids.
    let fields =
        "rss=,vsz=,etime=,nice=,utime=,stime=,time=,ppid=,pgid=,state=,ucomm=,gid=,uid=,user=,args=";
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", fields])
        .output();
    let Ok(o) = out else { return Ok(Value::Nil) };
    let text = decode(&o.stdout);
    let Some(line) = text.lines().next() else {
        return Ok(Value::Nil);
    };
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 15 {
        return Ok(Value::Nil);
    }
    let args_str = line
        .split_whitespace()
        .skip(14)
        .collect::<Vec<_>>()
        .join(" ");
    let num = |s: &str| Value::Int(s.parse::<i128>().unwrap_or(0));
    let time_of = |s: &str| -> Value {
        // "[[dd-]hh:]mm:ss[.frac]" → GNU (hi lo usec psec) time list.
        let usec = (ps_time_secs_f(s) * 1_000_000.0) as i128;
        time_list(usec)
    };
    let now_us: i128 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i128)
        .unwrap_or(0);
    let etime_us = (ps_time_secs_f(f[2]) * 1_000_000.0) as i128;
    // GNU's `comm` is the bare executable name — `ucomm` gives the
    // accounting name without the path.
    let comm = f[10].to_string();
    let gid_n = f[11].parse::<libc::gid_t>().unwrap_or(0);
    // macOS ps prints the numeric gid for `group` — resolve the name.
    let group = unsafe {
        let gr = libc::getgrgid(gid_n);
        if gr.is_null() {
            f[11].to_string()
        } else {
            std::ffi::CStr::from_ptr((*gr).gr_name)
                .to_string_lossy()
                .into_owned()
        }
    };
    let pair = |i: &mut Interp, k: &str, v: Value| Value::cons(symv(i, k), v);
    let items = vec![
        pair(i, "args", Value::string(args_str)),
        pair(i, "thcount", Value::Int(1)),
        pair(i, "rss", num(f[0])),
        pair(i, "vsize", num(f[1])),
        pair(i, "etime", time_list(etime_us)),
        pair(i, "start", time_list(now_us - etime_us)),
        pair(i, "nice", num(f[3])),
        pair(i, "majflt", Value::Int(0)),
        pair(i, "time", time_of(f[6])),
        pair(i, "stime", time_of(f[5])),
        pair(i, "utime", time_of(f[4])),
        pair(i, "tpgid", Value::Int(0)),
        pair(i, "pgrp", num(f[8])),
        pair(i, "ppid", num(f[7])),
        pair(i, "state", Value::string(f[9])),
        pair(i, "comm", Value::string(comm)),
        pair(i, "group", Value::string(group)),
        pair(i, "egid", Value::Int(gid_n as i128)),
        pair(i, "user", Value::string(f[13].to_string())),
        pair(i, "euid", num(f[12])),
    ];
    Ok(Value::list(items))
}

/// GNU time list (HIGH LOW MICRO PICO) from total microseconds.
fn time_list(usec: i128) -> Value {
    let secs = usec.div_euclid(1_000_000);
    let micro = usec.rem_euclid(1_000_000);
    Value::list(vec![
        Value::Int(secs.div_euclid(65536)),
        Value::Int(secs.rem_euclid(65536)),
        Value::Int(micro),
        Value::Int(0),
    ])
}

/// Parse `[[dd-]hh:]mm:ss[.frac]` into fractional total seconds.
fn ps_time_secs_f(s: &str) -> f64 {
    let s = s.trim();
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<f64>().unwrap_or(0.0), r),
        None => (0.0, s),
    };
    let parts: Vec<&str> = rest.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [a, b, c] => (
            a.parse::<f64>().unwrap_or(0.0),
            b.parse::<f64>().unwrap_or(0.0),
            c.parse::<f64>().unwrap_or(0.0),
        ),
        [a, b] => (
            0.0,
            a.parse::<f64>().unwrap_or(0.0),
            b.parse::<f64>().unwrap_or(0.0),
        ),
        _ => (0.0, 0.0, 0.0),
    };
    days * 86400.0 + h * 3600.0 + m * 60.0 + sec
}

// ---------- network interfaces ----------

unsafe fn sockaddr_to_value(sa: *const libc::sockaddr) -> Option<Value> {
    // SAFETY: callers pass valid ifaddrs pointers; derefs stay in unsafe block.
    if sa.is_null() {
        return None;
    }
    match unsafe { (*sa).sa_family } as i32 {
        libc::AF_INET => {
            let sin = unsafe { &*(sa as *const libc::sockaddr_in) };
            let b = sin.sin_addr.s_addr.to_ne_bytes();
            let port = u16::from_be(sin.sin_port) as i128;
            Some(Value::Vec(Rc::new(RefCell::new(vec![
                Value::Int(b[0] as i128),
                Value::Int(b[1] as i128),
                Value::Int(b[2] as i128),
                Value::Int(b[3] as i128),
                Value::Int(port),
            ]))))
        }
        libc::AF_INET6 => {
            let sin6 = unsafe { &*(sa as *const libc::sockaddr_in6) };
            let b = sin6.sin6_addr.s6_addr;
            let mut elems: Vec<Value> = Vec::with_capacity(9);
            for pair in b.chunks(2) {
                elems.push(Value::Int((((pair[0] as u16) << 8) | pair[1] as u16) as i128));
            }
            elems.push(Value::Int(u16::from_be(sin6.sin6_port) as i128));
            Some(Value::Vec(Rc::new(RefCell::new(elems))))
        }
        _ => None,
    }
}

fn f_network_interface_list(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let mut out = Vec::new();
    unsafe {
        let mut addrs: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut addrs) == 0 {
            let mut cur = addrs;
            while !cur.is_null() {
                let ifa = &*cur;
                if let Some(addr) = sockaddr_to_value(ifa.ifa_addr) {
                    let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                        .to_string_lossy()
                        .into_owned();
                    out.push(Value::cons(Value::string(name), addr));
                }
                cur = ifa.ifa_next;
            }
            libc::freeifaddrs(addrs);
        }
    }
    Ok(Value::list(out))
}

fn f_network_interface_info(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.wrong_type_mut("stringp", &a[0])),
    };
    let mut result: Option<(Value, Value, Value)> = None;
    unsafe {
        let mut addrs: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut addrs) == 0 {
            let mut cur = addrs;
            while !cur.is_null() {
                let ifa = &*cur;
                let iname = std::ffi::CStr::from_ptr(ifa.ifa_name)
                    .to_string_lossy()
                    .into_owned();
                if iname == name && (*ifa.ifa_addr).sa_family as i32 == libc::AF_INET {
                    let addr = sockaddr_to_value(ifa.ifa_addr).unwrap_or(Value::Nil);
                    let bcast = sockaddr_to_value(ifa.ifa_dstaddr).unwrap_or(Value::Nil);
                    let mask = sockaddr_to_value(ifa.ifa_netmask).unwrap_or(Value::Nil);
                    result = Some((addr, bcast, mask));
                }
                cur = ifa.ifa_next;
            }
            libc::freeifaddrs(addrs);
        }
    }
    match result {
        Some((addr, bcast, mask)) => Ok(Value::list(vec![addr, bcast, mask, Value::Nil])),
        None => Ok(Value::Nil),
    }
}

fn f_format_network_address(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let omit_port = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    match &a[0] {
        Value::Vec(v) => {
            let v = v.borrow();
            let nums: Vec<i128> = v.iter().filter_map(|x| x.int()).collect();
            let dotted = |b: &[i128]| {
                format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
            };
            let s = match nums.len() {
                4 => dotted(&nums),
                5 if !omit_port => format!("{}:{}", dotted(&nums), nums[4]),
                5 => dotted(&nums),
                9 if !omit_port => {
                    let parts: Vec<String> = nums[..8].iter().map(|n| format!("{n:x}")).collect();
                    format!("[{}]:{}", parts.join(":"), nums[8])
                }
                9 => {
                    let parts: Vec<String> = nums[..8].iter().map(|n| format!("{n:x}")).collect();
                    format!("[{}]", parts.join(":"))
                }
                8 => {
                    let parts: Vec<String> = nums[..8].iter().map(|n| format!("{n:x}")).collect();
                    parts.join(":")
                }
                _ => return Ok(Value::Nil),
            };
            Ok(Value::string(s))
        }
        Value::Cons(_) => {
            // GNU prints "<Family FAMILY>" for non-vector addresses.
            let items = a[0].list_to_vec().unwrap_or_default();
            let fam = items.first().and_then(|v| v.int()).unwrap_or(0);
            Ok(Value::string(format!("<Family {fam}>")))
        }
        _ => Ok(Value::Nil),
    }
}

// ---------- variables ----------

pub(crate) static SUBRS: &[Subr] = &[
    S!("make-process", many 0, f_make_process, "Start a subprocess (keyword args)."),
    S!("make-pipe-process", many 0, f_make_pipe_process, "Create an Emacs-internal pipe process."),
    S!("make-network-process", many 0, f_make_network_process, "Open a network connection or server."),
    S!("make-serial-process", many 0, f_make_serial_process, "Open a serial port process."),
    S!("serial-process-configure", many 0, f_serial_process_configure, "Configure serial port parameters."),
    S!("start-process", many 3, f_start_process, "Start PROGRAM in BUFFER (returns process)."),
    S!("processp", 1, 1, f_processp, "t if OBJECT is a process."),
    S!("get-process", 1, 1, f_get_process, "Process named NAME or PROCESS itself."),
    S!("process-list", 0, 0, f_process_list, "List of live processes."),
    S!("list-processes", 0, 1, f_list_processes, "Show a process list buffer."),
    S!("process-name", 1, 1, f_process_name, "Process name string."),
    S!("process-buffer", 1, 1, f_process_buffer, "Buffer associated with PROCESS."),
    S!("set-process-buffer", 2, 2, f_set_process_buffer, "Set the process buffer."),
    S!("get-buffer-process", 1, 1, f_get_buffer_process, "Process whose buffer is BUFFER."),
    S!("process-command", 1, 1, f_process_command, "Command list of PROCESS."),
    S!("process-type", 1, 1, f_process_type, "Type symbol: real/network/serial/pipe."),
    S!("process-id", 1, 1, f_process_id, "OS pid of PROCESS."),
    S!("process-status", 1, 1, f_process_status, "Status symbol of PROCESS."),
    S!("process-exit-status", 1, 1, f_process_exit_status, "Exit code or signal of PROCESS."),
    S!("process-mark", 1, 1, f_process_mark, "Marker at process-buffer insertion point."),
    S!("process-contact", 1, 2, f_process_contact, "Contact info of PROCESS."),
    S!("process-plist", 1, 1, f_process_plist, "Property list of PROCESS."),
    S!("set-process-plist", 2, 2, f_set_process_plist, "Set process property list."),
    S!("process-get", 2, 2, f_process_get, "Get PROP from process plist."),
    S!("process-put", 3, 3, f_process_put, "Set PROP on process plist."),
    S!("process-query-on-exit-flag", 1, 1, f_process_query_on_exit_flag, "t if Emacs queries on exit."),
    S!("set-process-query-on-exit-flag", 2, 2, f_set_process_query_on_exit_flag, "Set query-on-exit flag."),
    S!("process-filter", 1, 1, f_process_filter, "Filter function of PROCESS."),
    S!("set-process-filter", 2, 2, f_set_process_filter, "Set process filter."),
    S!("process-sentinel", 1, 1, f_process_sentinel, "Sentinel of PROCESS."),
    S!("set-process-sentinel", 2, 2, f_set_process_sentinel, "Set process sentinel."),
    S!("process-tty-name", 1, 1, f_process_tty_name, "Controlling tty name or nil."),
    S!("process-coding-system", 1, 1, f_process_coding_system, "(DECODE . ENCODE) coding systems."),
    S!("set-process-coding-system", 1, 3, f_set_process_coding_system, "Set process coding systems."),
    S!("process-inherit-coding-system-flag", 1, 1, f_process_inherit_coding_system_flag, "Inherit-coding flag."),
    S!("set-process-inherit-coding-system-flag", 2, 2, f_set_process_inherit_coding_system_flag, "Set inherit-coding flag."),
    S!("set-process-window-size", 3, 3, f_set_process_window_size, "Tell PROCESS its window size."),
    S!("process-datagram-address", 1, 1, f_process_datagram_address, "Datagram address or nil."),
    S!("set-process-datagram-address", 2, 2, f_set_process_datagram_address, "Set datagram address."),
    S!("accept-process-output", 0, 4, f_accept_process_output, "Read pending process output."),
    S!("process-send-string", 2, 2, f_process_send_string, "Send STRING to PROCESS."),
    S!("process-send-region", 3, 3, f_process_send_region, "Send region text to PROCESS."),
    S!("process-send-eof", 1, 1, f_process_send_eof, "Send EOF to PROCESS."),
    S!("delete-process", 1, 1, f_delete_process, "Delete PROCESS (kill if live)."),
    S!("signal-process", 2, 2, f_signal_process, "Send SIGNAL to process or pid."),
    S!("interrupt-process", 0, 2, f_interrupt_process, "Send SIGINT to PROCESS."),
    S!("kill-process", 0, 2, f_kill_process, "Send SIGKILL to PROCESS."),
    S!("quit-process", 0, 2, f_quit_process, "Send SIGQUIT to PROCESS."),
    S!("stop-process", 0, 2, f_stop_process, "Suspend PROCESS (SIGTSTP)."),
    S!("continue-process", 0, 2, f_continue_process, "Resume PROCESS (SIGCONT)."),
    S!("process-running-child-p", 0, 1, f_process_running_child_p, "Whether PROCESS has a running child."),
    S!("internal-default-process-filter", 2, 2, f_internal_default_process_filter, "Default filter: insert into buffer."),
    S!("internal-default-process-sentinel", 2, 2, f_nil2, "Default sentinel (no-op)."),
    S!("process-lines", many 1, f_process_lines, "Run PROGRAM, return output lines."),
    S!("process-lines-ignore-status", many 1, f_process_lines_ignore_status, "process-lines ignoring exit status."),
    S!("list-system-processes", 0, 0, f_list_system_processes, "Pids of all system processes."),
    S!("process-attributes", 1, 1, f_process_attributes, "Alist of attributes of PID."),
    S!("network-interface-list", 0, 0, f_network_interface_list, "Network interfaces and addresses."),
    S!("network-interface-info", 1, 1, f_network_interface_info, "(ADDR BCAST MASK HW) for IFNAME."),
    S!("format-network-address", 1, 2, f_format_network_address, "Format ADDRESS vector as string."),
];

fn f_nil2(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
