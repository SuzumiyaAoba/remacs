//! Subprocess and IPC object: `Value::Process` plus the Emacs process API.
//!
//! GNU Emacs supports real subprocesses, pipe processes, network
//! connections, serial ports, and listening servers. We implement the same
//! Lisp-visible model on top of `std::process`/`std::net`/libc: a `Proc`
//! records the command, status, buffer/filter/sentinel destinations, and
//! the underlying I/O handle. `accept-process-output` is the poll point —
//! it drains pending output into filters or process buffers and runs
//! sentinels on status transitions.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::Rc;

mod child;
mod coding;

use super::builtins::evalfn::timer_check;
use super::builtins::{S, arg};
use super::error::Flow;
use super::eval::plist_get;
use super::obarray::sym;
use super::value::{Marker, MarkerRef, ProcessRef, Subr, SymId, Value};
use super::{EvalResult, Interp};

/// OS-level I/O behind a process object.
pub enum ProcIo {
    /// A real child with independently selected pipe/pty input and output.
    Child {
        child: std::process::Child,
        input_fd: Option<OwnedFd>,
        output_fd: Option<OwnedFd>,
        tty_fd: Option<OwnedFd>,
        pty_input: bool,
        pty_output: bool,
        stderr: Option<std::process::ChildStderr>,
        /// Unsent input, shared with reentrant sends from filters/timers.
        pending_input: VecDeque<u8>,
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
    /// A nonblocking TCP connect, completed only by the process event loop.
    Connecting(TcpConnect),
    /// A message-oriented socket. Never use stream read/write semantics here.
    Datagram(DatagramIo),
    /// A listening server socket.
    Listen(std::net::TcpListener),
    /// A connected local (Unix-domain) stream — `:family local'.
    LocalNet(std::os::unix::net::UnixStream),
    /// A listening local socket and its filesystem path (unlinked on
    /// drop, like GNU removing the socket file).
    LocalListen(std::os::unix::net::UnixListener, String),
    /// A serial port (file-backed).
    Serial(std::fs::File),
    /// No backend — GNU allows `make-process` without :command.
    None,
}

/// Keep ownership of an in-flight socket, including the untried addresses.
/// Dropping this state cancels the attempt without waiting for the network.
pub struct TcpConnect {
    socket: Option<std::net::TcpStream>,
    remote: std::net::SocketAddr,
    remaining: VecDeque<std::net::SocketAddr>,
}

impl std::os::fd::AsRawFd for TcpConnect {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.socket.as_ref().expect("connecting socket").as_raw_fd()
    }
}

/// Set O_NONBLOCK before connect(2), not after TcpStream::connect has waited.
/// The TcpStream owns the descriptor immediately, including all error paths.
fn nonblocking_tcp_connect(addr: std::net::SocketAddr) -> std::io::Result<std::net::TcpStream> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let fd = unsafe {
        libc::socket(
            if addr.is_ipv6() {
                libc::AF_INET6
            } else {
                libc::AF_INET
            },
            libc::SOCK_STREAM,
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let socket = unsafe { std::net::TcpStream::from_raw_fd(fd) };
    if unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    socket.set_nonblocking(true)?;
    let result = match addr {
        std::net::SocketAddr::V4(addr) => {
            let mut raw: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            raw.sin_family = libc::AF_INET as libc::sa_family_t;
            raw.sin_port = addr.port().to_be();
            raw.sin_addr.s_addr = u32::from_ne_bytes(addr.ip().octets());
            #[cfg(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "dragonfly"
            ))]
            {
                raw.sin_len = std::mem::size_of_val(&raw) as u8;
            }
            unsafe {
                libc::connect(
                    fd,
                    (&raw as *const libc::sockaddr_in).cast(),
                    std::mem::size_of_val(&raw) as libc::socklen_t,
                )
            }
        }
        std::net::SocketAddr::V6(addr) => {
            let mut raw: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
            raw.sin6_family = libc::AF_INET6 as libc::sa_family_t;
            raw.sin6_port = addr.port().to_be();
            raw.sin6_addr.s6_addr = addr.ip().octets();
            raw.sin6_flowinfo = addr.flowinfo();
            raw.sin6_scope_id = addr.scope_id();
            #[cfg(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "dragonfly"
            ))]
            {
                raw.sin6_len = std::mem::size_of_val(&raw) as u8;
            }
            unsafe {
                libc::connect(
                    fd,
                    (&raw as *const libc::sockaddr_in6).cast(),
                    std::mem::size_of_val(&raw) as libc::socklen_t,
                )
            }
        }
    };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.raw_os_error(),
            Some(libc::EINPROGRESS | libc::EINTR | libc::EALREADY)
        ) {
            return Err(error);
        }
    }
    Ok(socket)
}

impl TcpConnect {
    fn start(mut remaining: VecDeque<std::net::SocketAddr>) -> std::io::Result<Self> {
        let mut error = std::io::Error::from_raw_os_error(libc::EADDRNOTAVAIL);
        while let Some(addr) = remaining.pop_front() {
            match nonblocking_tcp_connect(addr) {
                Ok(socket) => {
                    return Ok(Self {
                        socket: Some(socket),
                        remote: addr,
                        remaining,
                    });
                }
                Err(e) => error = e,
            }
        }
        Err(error)
    }

    /// SO_ERROR is meaningful only after readiness. Reading it too early can
    /// report zero while the handshake is still in progress.
    fn poll(&mut self) -> std::io::Result<Option<std::net::TcpStream>> {
        use std::os::fd::AsRawFd;
        let socket = self.socket.as_ref().expect("connecting socket");
        let mut fd = libc::pollfd {
            fd: socket.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut fd, 1, 0) };
        if ready == 0 {
            return Ok(None);
        }
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            return if error.kind() == std::io::ErrorKind::Interrupted {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let result = socket.take_error().and_then(|error| match error {
            Some(error) => Err(error),
            None => socket.peer_addr().map(|_| ()),
        });
        match result {
            Ok(()) => Ok(self.socket.take()),
            Err(error) => {
                self.socket.take();
                if self.remaining.is_empty() {
                    return Err(error);
                }
                // Each replacement is also nonblocking. Defer completion to
                // the next event-loop turn instead of spinning on readiness.
                *self = Self::start(std::mem::take(&mut self.remaining))?;
                Ok(None)
            }
        }
    }
}

/// The current UDP/local-datagram destination is updated by each received packet.
/// Keep it separate from the socket: GNU uses sendto/recvfrom, not connect.
#[derive(Clone)]
enum DatagramAddress {
    Inet(std::net::SocketAddr),
    Local(String),
}

pub enum DatagramSocket {
    Inet(std::net::UdpSocket),
    Local(std::os::unix::net::UnixDatagram),
}

pub struct DatagramIo {
    pub socket: DatagramSocket,
    remote: Option<DatagramAddress>,
    /// sockaddr size, also used for GNU's initial unspecified remote address.
    address_size: usize,
    /// GNU snapshots read-process-output-max when the socket is created.
    read_max: usize,
    /// UTF-8 decoders retain incomplete characters across receive calls.
    /// Only unlink a pathname this process successfully bound itself.
    bound_path: Option<String>,
    /// Whole messages survive nonblocking retries; GNU uses the current peer.
    pending: VecDeque<Vec<u8>>,
}

impl Drop for DatagramIo {
    fn drop(&mut self) {
        if let Some(path) = &self.bound_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl DatagramAddress {
    fn value(&self) -> Value {
        match self {
            Self::Inet(addr) => socket_addr_value(*addr),
            Self::Local(path) => Value::string(path.clone()),
        }
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(Self::Local(s.borrow().clone())),
            Value::Vec(v) => {
                let v = v.borrow();
                // GNU masks address components to their network field width.
                let part = |n: usize| v[n].int().unwrap_or(0);
                let addr = match v.len() {
                    5 => {
                        let mut octets = [0u8; 4];
                        for (slot, value) in
                            octets.iter_mut().zip(v[..4].iter().filter_map(Value::int))
                        {
                            *slot = value as u8;
                        }
                        std::net::SocketAddr::from((octets, part(4) as u16))
                    }
                    9 => std::net::SocketAddr::from((
                        [
                            part(0) as u16,
                            part(1) as u16,
                            part(2) as u16,
                            part(3) as u16,
                            part(4) as u16,
                            part(5) as u16,
                            part(6) as u16,
                            part(7) as u16,
                        ],
                        part(8) as u16,
                    )),
                    _ => return None,
                };
                Some(Self::Inet(addr))
            }
            _ => None,
        }
    }

    fn size(&self) -> usize {
        match self {
            Self::Inet(std::net::SocketAddr::V4(_)) => std::mem::size_of::<libc::sockaddr_in>(),
            Self::Inet(std::net::SocketAddr::V6(_)) => std::mem::size_of::<libc::sockaddr_in6>(),
            Self::Local(_) => std::mem::size_of::<libc::sockaddr_un>(),
        }
    }
}

impl DatagramIo {
    fn remote_value(&self) -> Value {
        self.remote
            .as_ref()
            .map(DatagramAddress::value)
            .unwrap_or_else(|| {
                // GNU's server starts with an all-zero sockaddr (AF_UNSPEC), not
                // its own listening address. Account for BSD's leading sa_len.
                let header = std::mem::offset_of!(libc::sockaddr, sa_family)
                    + std::mem::size_of::<libc::sa_family_t>();
                Value::cons(
                    Value::Int(0),
                    Value::vector(vec![Value::Int(0); self.address_size - header]),
                )
            })
    }

    fn recv(&self, bytes: &mut [u8]) -> std::io::Result<(usize, DatagramAddress)> {
        match &self.socket {
            DatagramSocket::Inet(s) => s
                .recv_from(bytes)
                .map(|(n, addr)| (n, DatagramAddress::Inet(addr))),
            DatagramSocket::Local(s) => s.recv_from(bytes).map(|(n, addr)| {
                (
                    n,
                    DatagramAddress::Local(
                        addr.as_pathname()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                )
            }),
        }
    }

    fn send(&self, bytes: &[u8], remote: &Option<DatagramAddress>) -> std::io::Result<usize> {
        match (&self.socket, remote) {
            (DatagramSocket::Inet(s), Some(DatagramAddress::Inet(addr))) => s.send_to(bytes, addr),
            (DatagramSocket::Local(s), Some(DatagramAddress::Local(path))) => {
                s.send_to(bytes, path)
            }
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }
}

fn socket_addr_value(addr: std::net::SocketAddr) -> Value {
    let mut parts: Vec<Value> = match addr.ip() {
        std::net::IpAddr::V4(ip) => ip.octets().iter().map(|n| Value::Int(*n as i128)).collect(),
        std::net::IpAddr::V6(ip) => ip
            .segments()
            .iter()
            .map(|n| Value::Int(*n as i128))
            .collect(),
    };
    parts.push(Value::Int(addr.port() as i128));
    Value::vector(parts)
}

impl Drop for ProcIo {
    /// Close leaked fds and reap children so an Interp (or dropped
    /// process object) can't exhaust the system's pty table — the unit
    /// suite runs hundreds of procs in one OS process.
    fn drop(&mut self) {
        match self {
            ProcIo::Child {
                child, input_fd, output_fd, tty_fd, ..
            } => {
                let _ = child.kill();
                let _ = child.wait();
                input_fd.take();
                output_fd.take();
                tty_fd.take();
            }
            ProcIo::Pipe {
                read_fd,
                child_wfd,
                sink_fd,
            } => {
                for fd in [*read_fd, *child_wfd, *sink_fd] {
                    if fd >= 0 {
                        unsafe { libc::close(fd) };
                    }
                }
            }
            ProcIo::LocalListen(_, path) => {
                let _ = std::fs::remove_file(path);
            }
            _ => {}
        }
    }
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
    /// GNU stores the file-error data when connect fails before it can be
    /// polled (as opposed to an asynchronous SO_ERROR errno).
    connect_error: Value,
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
    codecs: coding::State,
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
    /// The open sentinel is running; nested waits must not read this socket
    /// until it returns, though sending from that sentinel is permitted.
    opening: bool,
    /// :stop given at creation — start stopped.
    pub start_stopped: bool,
    /// Signal sent but not yet reflected in `status` (GNU applies
    /// status changes asynchronously via SIGCHLD notification).
    pub pending_status: Option<&'static str>,
    /// GnuTLS state for TLS network processes (`gnutls-boot').
    pub gnutls: Option<Box<GnutlsState>>,
    /// GNU `p->gnutls_init_stage' — 0 until `gnutls-boot' progresses.
    pub gnutls_initstage: i32,
    /// `gnutls-log-level' in effect when the session was booted.
    pub gnutls_log_level: i32,
    /// Async-negotiation markers (`gnutls-asynchronous-parameters').
    pub gnutls_async_connected: Value,
    pub gnutls_async_signalled: Value,
}

/// Per-process GnuTLS session/credential handles (`lisp.h' fields).
pub struct GnutlsState {
    /// `gnutls_session_t'.
    pub session: *mut core::ffi::c_void,
    /// `gnutls_certificate_client_credentials' (x509 or anon).
    pub cred: *mut core::ffi::c_void,
    /// Hostname used for SNI + certificate verification.
    pub hostname: String,
    /// Raw status bits from the last `gnutls_certificate_verify_peers2'.
    pub verify_status: u32,
    /// Peer certificate chain as imported DER blobs.
    pub certificates: Vec<Vec<u8>>,
    /// Peer certificate did not match :hostname (extra verification).
    pub host_mismatch: bool,
    /// Credentials were allocated for `gnutls-anon' (not x509pki).
    pub anon: bool,
}

impl Proc {
    pub(crate) fn gc_values(&self) -> [&Value; 11] {
        [
            &self.command,
            &self.connect_error,
            &self.filter,
            &self.sentinel,
            &self.plist,
            &self.tty_name,
            &self.stderr_dest,
            &self.connection_type,
            &self.contact,
            &self.gnutls_async_connected,
            &self.gnutls_async_signalled,
        ]
    }

    pub fn alive(&self) -> bool {
        matches!(self.status, "run" | "stop" | "open" | "listen" | "connect")
    }

    /// The live `gnutls_session_t', if a TLS session was booted.
    pub fn gnutls_session(&self) -> Option<*mut core::ffi::c_void> {
        self.gnutls.as_ref().map(|s| s.session)
    }

    pub fn gnutls_hostname(&self) -> Option<String> {
        self.gnutls.as_ref().map(|s| s.hostname.clone())
    }
}

/// GNU `CHECK_PROCESS': only a process object — names and buffers do
/// NOT resolve here (that's `get_process''s job, used by the control
/// verbs, not the accessors).
fn want_proc(i: &mut Interp, v: &Value) -> Result<ProcessRef, Flow> {
    match v {
        Value::Process(p) => Ok(p.clone()),
        _ => Err(i.wrong_type_mut("processp", v)),
    }
}

/// GNU `Fget_process': a process object or a name lookup in
/// Vprocess_alist; anything else signals stringp.  Returns nil for
/// an unknown name (no error).
pub(crate) fn proc_by_name(i: &mut Interp, v: &Value) -> Result<Option<ProcessRef>, Flow> {
    match v {
        Value::Process(p) => Ok(Some(p.clone())),
        Value::Str(s) => {
            let name = s.borrow().clone();
            Ok(i.processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().name == name)
                .cloned())
        }
        other => Err(i.wrong_type_mut("stringp", other)),
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

fn coding_value(i: &mut Interp, name: &str) -> Value {
    if name == "nil" {
        Value::Nil
    } else {
        symv(i, name)
    }
}

fn buffer_multibyte(i: &mut Interp, buffer: usize) -> bool {
    let variable = i.intern("enable-multibyte-characters");
    i.buffers
        .get(buffer)
        .and_then(|b| b.borrow().locals.get(&variable).cloned())
        .unwrap_or_else(|| i.obarray.symbol(variable).value.clone())
        .truthy()
}

fn decode_process_output(
    i: &mut Interp,
    process: &ProcessRef,
    bytes: &[u8],
    stderr: bool,
    finish: bool,
    buffer: Option<usize>,
) -> EvalResult {
    let default_filter = {
        let p = process.borrow();
        p.filter.is_nil()
            || matches!(p.filter, Value::Sym(id) if id == i.intern("internal-default-process-filter"))
    };
    let raw =
        buffer.is_some_and(|buffer| (stderr || default_filter) && !buffer_multibyte(i, buffer));
    let (value, used) = {
        let mut p = process.borrow_mut();
        let name = if stderr {
            p.codecs.stderr_name.clone()
        } else {
            p.coding.0.clone()
        };
        let state = if stderr {
            &mut p.codecs.error
        } else {
            &mut p.codecs.output
        };
        let (value, used) = state.decode(i, &name, bytes, finish, raw)?;
        if stderr {
            p.codecs.stderr_name = used.clone();
        } else {
            p.coding.0 = used.clone();
        }
        (value, used)
    };
    let coding = coding_value(i, &used);
    let last = i.intern("last-coding-system-used");
    i.obarray.symbol_mut(last).value = coding.clone();
    if value.is_nil() {
        return Ok(value);
    }
    super::builtins::misc::process_decode_result(i, value, &coding)
}

fn encode_process_input(
    i: &mut Interp,
    process: &ProcessRef,
    value: Value,
) -> Result<Vec<u8>, Flow> {
    prepare_process_write(i, process)?;
    let multibyte = super::builtins::strfn::string_multibyte_p(i, &value);
    let mut name = process.borrow().coding.1.clone();
    let mut coding = coding_value(i, &name);
    if multibyte {
        name = coding::encoding_name(i, &coding)?;
        coding = coding_value(i, &name);
        process.borrow_mut().coding.1 = name.clone();
    }
    let value = if multibyte {
        super::builtins::misc::process_encode_source(i, value, &coding)?
    } else {
        value
    };
    let bytes = process.borrow_mut().codecs.input.encode(i, &name, value)?;
    let last = i.intern("last-coding-system-used");
    i.obarray.symbol_mut(last).value = coding;
    Ok(bytes)
}

fn insert_coded_into_buffer(i: &mut Interp, buffer: usize, position: Option<usize>, value: &Value) {
    let Value::Str(source) = value else { return };
    let multi = buffer_multibyte(i, buffer);
    let unibyte = i.is_unibyte_str(source);
    let text: String = source
        .borrow()
        .chars()
        .map(|c| {
            if !multi {
                super::value::eight_bit_byte(c).unwrap_or(c as u8) as char
            } else if unibyte && (128..=255).contains(&(c as u32)) {
                super::value::lisp_char(0x3fff00 + c as u32).unwrap()
            } else {
                c
            }
        })
        .collect();
    let _ = crate::buffer::primitives::chg_with_buffer(i, buffer, |i| {
        let at = position
            .unwrap_or_else(|| cur_text_len(i))
            .min(cur_text_len(i));
        crate::buffer::primitives::chg_insert(i, at, &text)?;
        crate::buffer::primitives::copy_str_props(i, source, at);
        Ok(Value::Nil)
    });
}

/// Decode-process-output hook point: raw bytes → string.
/// We keep this UTF-8-lossy like GNU's default undecided/utf-8 handling.
fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// ---------- output / status engine ----------

/// Feed process output to its filter or buffer (no borrow held).
fn deliver_output(
    i: &mut Interp,
    pref: &ProcessRef,
    out: Vec<u8>,
    to_stderr_dest: bool,
) -> Result<(), Flow> {
    if out.is_empty() {
        return Ok(());
    }
    let sdest = pref.borrow().stderr_dest.clone();
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
                    let value = decode_process_output(i, pref, &out, true, false, Some(bid))?;
                    insert_coded_into_buffer(i, bid, None, &value);
                }
            }
        }
        return Ok(());
    }
    let buffer = pref.borrow().buffer;
    let value = decode_process_output(i, pref, &out, false, false, buffer)?;
    deliver_decoded(i, pref, value)
}

fn deliver_decoded(i: &mut Interp, pref: &ProcessRef, value: Value) -> Result<(), Flow> {
    let (filter, buffer, mark) = {
        let p = pref.borrow();
        (p.filter.clone(), p.buffer, p.mark.clone())
    };
    // An incomplete multibyte character does not invoke a filter yet.
    if value.is_nil() {
        return Ok(());
    }
    let text = str_value(&value).unwrap_or_default();
    if filter.truthy() {
        i.apply(&filter, vec![Value::Process(pref.clone()), value])?;
        return Ok(());
    }
    if let Some(bid) = buffer {
        // Insert at the process mark, advancing it like GNU does.
        let pos = mark
            .as_ref()
            .map(|m| m.borrow().position)
            .unwrap_or_else(|| {
                i.buffers
                    .get(bid)
                    .map(|b| b.borrow().text_len())
                    .unwrap_or(0)
            });
        let n = text.chars().count();
        insert_coded_into_buffer(i, bid, Some(pos), &value);
        if let Some(m) = &mark {
            m.borrow_mut().position = pos + n;
        }
    }
    Ok(())
}


fn cur_text_len(i: &mut Interp) -> usize {
    crate::buffer::primitives::cur(i).borrow().text_len()
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
        "failed" => format!("failed with code {code}\n"),
        "closed" if code == 256 => "connection broken by remote peer\n".to_string(),
        _ => format!("{status}\n"),
    }
}

/// Deliver a status change through the installed sentinel.
fn run_sentinel(i: &mut Interp, pref: &ProcessRef, status: &str, code: i32) -> Result<(), Flow> {
    let mut sentinel = pref.borrow().sentinel.clone();
    if sentinel.is_nil() {
        sentinel = symv(i, "internal-default-process-sentinel");
    }
    i.apply(
        &sentinel,
        vec![
            Value::Process(pref.clone()),
            Value::string(event_string(status, code)),
        ],
    )?;
    Ok(())
}

/// Poll one process: drain output, observe exit. Returns true if anything
/// was consumed/changed.
fn poll_proc(i: &mut Interp, pref: &ProcessRef) -> Result<bool, Flow> {
    if pref.borrow().dead || pref.borrow().opening {
        return Ok(false);
    }
    if matches!(pref.borrow().io, ProcIo::Connecting(_)) {
        return poll_connect(i, pref);
    }
    {
        let p = pref.borrow();
        if matches!(p.io, ProcIo::Net(_))
            && (p.status == "stop" || matches!(p.filter, Value::Sym(s) if s == sym::T))
        {
            return Ok(false);
        }
    }
    if matches!(pref.borrow().io, ProcIo::Datagram(_)) {
        return poll_datagram(i, pref);
    }
    enum Ev {
        Out(Vec<u8>, bool),
        Exit(&'static str, i32),
        Accepted(std::net::TcpStream, std::net::SocketAddr),
        AcceptedLocal(std::os::unix::net::UnixStream),
    }
    let mut events = Vec::new();
    {
        let mut p = pref.borrow_mut();
        // TLS sessions read through gnutls_record_recv instead of the
        // raw socket (GNU's emacs_gnutls_read); handled here because
        // the gnutls state lives outside `p.io'.
        if matches!(p.io, ProcIo::Net(_)) && crate::lisp::builtins::gnutls::tls_ready(&p) {
            let mut buf = [0u8; 8192];
            loop {
                match crate::lisp::builtins::gnutls::record_read(&p, &mut buf) {
                    n if n > 0 => {
                        events.push(Ev::Out(buf[..n as usize].to_vec(), false));
                        continue;
                    }
                    0 => {
                        // Peer sent close_notify (or a fatal
                        // error) — GNU's `closed' transition.
                        if p.status != "closed" {
                            p.status = "closed";
                            p.exit_status = 0;
                            events.push(Ev::Exit("closed", 0));
                        }
                        break;
                    }
                    _ => break, // would-block
                }
            }
        } else {
            match &mut p.io {
                ProcIo::Child {
                    child,
                    input_fd,
                    output_fd,
                    tty_fd,
                    stderr,
                    pending_input,
                    ..
                } => {
                    // Observe exit before draining. If the child exits just
                    // after an EAGAIN read, leave it for the next poll so
                    // closing the channel cannot discard its final bytes.
                    let exited = child.try_wait();
                    let mut buf = [0u8; 8192];
                    loop {
                        let n = unsafe {
                            libc::read(
                                output_fd.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                                buf.as_mut_ptr() as *mut _,
                                buf.len(),
                            )
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
                    match exited {
                        Ok(Some(st)) if events.is_empty() => {
                            // Child is reaped; release both data channels and
                            // the optional terminal control descriptor.
                            input_fd.take();
                            output_fd.take();
                            tty_fd.take();
                            stderr.take();
                            *pending_input = VecDeque::new();
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
                ProcIo::Pipe {
                    read_fd, child_wfd, ..
                } => {
                    let mut buf = [0u8; 8192];
                    loop {
                        let n =
                            unsafe { libc::read(*read_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
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
                                // Remote closed the connection — GNU marks
                                // network processes `closed' (a routine
                                // transition: the default sentinel stays
                                // silent).
                                if p.status != "closed" {
                                    p.status = "closed";
                                    p.exit_status = 0;
                                    events.push(Ev::Exit("closed", 0));
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
                ProcIo::LocalNet(s) => {
                    let mut buf = [0u8; 8192];
                    loop {
                        match s.read(&mut buf) {
                            Ok(0) => {
                                // Same `closed' transition as TCP sockets.
                                if p.status != "closed" {
                                    p.status = "closed";
                                    p.exit_status = 0;
                                    events.push(Ev::Exit("closed", 0));
                                }
                                break;
                            }
                            Ok(n) => events.push(Ev::Out(buf[..n].to_vec(), false)),
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(_) => break,
                        }
                    }
                }
                ProcIo::LocalListen(l, _) => loop {
                    match l.accept() {
                        Ok((stream, _addr)) => events.push(Ev::AcceptedLocal(stream)),
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
                ProcIo::Datagram(_) => unreachable!("datagrams have a message-oriented poll path"),
                ProcIo::Connecting(_) => unreachable!("connects have a readiness poll path"),
                ProcIo::None => {}
            }
        }
    }
    // accept-process-output returns t only when new output data
    // arrived — exits and accepts don't count in GNU.
    let did = events.iter().any(|e| matches!(e, Ev::Out(..)));
    for ev in events {
        match ev {
            Ev::Out(bytes, stderr) => deliver_output(i, pref, bytes, stderr)?,
            Ev::Exit(st, code) => {
                let buffer = pref.borrow().buffer;
                let tail = decode_process_output(i, pref, &[], false, true, buffer)?;
                deliver_decoded(i, pref, tail)?;
                let stderr = pref.borrow().stderr_dest.clone();
                if let Some(buffer) = i.buffer_id_of(&stderr) {
                    let tail = decode_process_output(i, pref, &[], true, true, Some(buffer))?;
                    insert_coded_into_buffer(i, buffer, None, &tail);
                }

                // GNU's status_notify: a process reaching `exit',
                // `signal' or `closed' is removed from the process list
                // (delete-exited-processes defaults to t) BEFORE the
                // sentinel runs — the sentinel already sees it gone.
                // Only the oldest alist cell goes, matching
                // remove_process on Vprocess_alist.
                if matches!(pref.borrow().status, "exit" | "signal" | "closed") {
                    if let Some(pos) = i.processes.iter().position(|q| Rc::ptr_eq(q, pref)) {
                        i.processes.remove(pos);
                    }
                }
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

                // GNU's accepted process inherits the listener's
                // filter, sentinel and plist (server-process-filter
                // runs on the connection, not the listener).
                let (filter, sentinel, plist, coding, inherit_coding) = {
                    let pb = pref.borrow();
                    (
                        pb.filter.clone(),
                        pb.sentinel.clone(),
                        pb.plist.clone(),
                        pb.coding.clone(),
                        pb.inherit_coding,
                    )
                };
                let child_proc = crate::lisp::memory::allocate(
                    Proc {
                        // GNU names the accepted process "<host:port>".
                        name: unique_process_name(i, &format!("<{addr}>")),
                        kind: "network",
                        command: Value::Nil,
                        io: ProcIo::Net(stream),
                        pid: 0,
                        status: "open",
                        exit_status: 0,
                        connect_error: Value::Nil,
                        buffer,
                        mark: Some(free_process_marker()),
                        filter,
                        sentinel,
                        plist,
                        query_on_exit: true,
                        kill_without_query: false,
                        tty_name: Value::Nil,
                        coding,
                        codecs: Default::default(),
                        stderr_dest: Value::Nil,
                        inherit_coding,
                        connection_type: Value::Nil,
                        contact,
                        dead: false,
                        reported: false,
                        opening: false,
                        start_stopped: false,
                        pending_status: None,
                        gnutls: None,
                        gnutls_initstage: 0,
                        gnutls_log_level: 0,
                        gnutls_async_connected: Value::Nil,
                        gnutls_async_signalled: Value::Nil,
                    },
                    crate::lisp::Value::Process,
                );
                i.processes.push(child_proc);
                run_sentinel(i, pref, "open", 0)?;
            }
            Ev::AcceptedLocal(stream) => {
                // Local-socket accept — GNU names the child after the
                // peer's socket address when it has one.
                let _ = stream.set_nonblocking(true);
                let buffer = None;
                let name = stream
                    .peer_addr()
                    .ok()
                    .and_then(|a| a.as_pathname().map(|p| p.display().to_string()))
                    .map(|p| format!("<{p}>"))
                    .unwrap_or_else(|| "<local socket>".to_string());
                let contact = Value::list(vec![Value::string(name.clone())]);
                // Same GNU inheritance rule as the TCP accept path.
                let (filter, sentinel, plist, coding, inherit_coding) = {
                    let pb = pref.borrow();
                    (
                        pb.filter.clone(),
                        pb.sentinel.clone(),
                        pb.plist.clone(),
                        pb.coding.clone(),
                        pb.inherit_coding,
                    )
                };
                let child_proc = crate::lisp::memory::allocate(
                    Proc {
                        name: unique_process_name(i, &name),
                        kind: "network",
                        command: Value::Nil,
                        io: ProcIo::LocalNet(stream),
                        pid: 0,
                        status: "open",
                        exit_status: 0,
                        connect_error: Value::Nil,
                        buffer,
                        mark: Some(free_process_marker()),
                        filter,
                        sentinel,
                        plist,
                        query_on_exit: true,
                        kill_without_query: false,
                        tty_name: Value::Nil,
                        coding,
                        codecs: Default::default(),
                        stderr_dest: Value::Nil,
                        inherit_coding,
                        connection_type: Value::Nil,
                        contact,
                        dead: false,
                        reported: false,
                        opening: false,
                        start_stopped: false,
                        pending_status: None,
                        gnutls: None,
                        gnutls_initstage: 0,
                        gnutls_log_level: 0,
                        gnutls_async_connected: Value::Nil,
                        gnutls_async_signalled: Value::Nil,
                    },
                    crate::lisp::Value::Process,
                );
                i.processes.push(child_proc);
                run_sentinel(i, pref, "open", 0)?;
            }
        }
    }
    Ok(did)
}

/// Complete a nonblocking handshake before any filter can read from it.
/// Publish the new state and release the borrow before invoking Lisp: an open
/// sentinel can send, wait recursively, replace the filter, or delete p.
fn poll_connect(i: &mut Interp, pref: &ProcessRef) -> Result<bool, Flow> {
    let result = {
        let mut p = pref.borrow_mut();
        let ProcIo::Connecting(connect) = &mut p.io else {
            return Ok(false);
        };
        connect.poll()
    };
    match result {
        Ok(None) => {}
        Ok(Some(socket)) => {
            let local = socket.local_addr().ok().map(socket_addr_value);
            let remote = socket.peer_addr().ok().map(socket_addr_value);
            let local_kw = i.intern(":local");
            let remote_kw = i.intern(":remote");
            {
                let mut p = pref.borrow_mut();
                p.io = ProcIo::Net(socket);
                if p.status == "stop" {
                    p.pending_status = Some("open");
                } else {
                    p.status = "open";
                }
                p.opening = true;
                // Replace existing address keys, preserving the creation args.
                let mut items = p.contact.list_to_vec().unwrap_or_default();
                for (key, value) in [(local_kw, local), (remote_kw, remote)] {
                    if let Some(value) = value {
                        if let Some(at) = items
                            .chunks_exact(2)
                            .position(|pair| matches!(pair[0], Value::Sym(s) if s == key))
                        {
                            items[at * 2 + 1] = value;
                        } else {
                            items.extend([Value::Sym(key), value]);
                        }
                    }
                }
                p.contact = Value::list(items);
            }
            let result = run_sentinel(i, pref, "open", 0);
            pref.borrow_mut().opening = false;
            result?;
        }
        Err(error) => {
            let code = error.raw_os_error().unwrap_or(libc::EIO);
            {
                let mut p = pref.borrow_mut();
                p.io = ProcIo::None;
                if p.status == "stop" {
                    p.pending_status = Some("failed");
                } else {
                    p.status = "failed";
                }
                p.exit_status = code;
                p.reported = true;
            }
            // Unlike exit/closed, GNU keeps a failed connection registered
            // until explicitly deleted, including while its sentinel runs.
            run_sentinel(i, pref, "failed", code)?;
        }
    }
    // A status transition is not output for accept-process-output.
    Ok(false)
}

/// Deliver one packet before reading the next: filters must observe its sender,
/// and a reentrant wait must not dispatch newer packets ahead of buffered ones.
fn poll_datagram(i: &mut Interp, pref: &ProcessRef) -> Result<bool, Flow> {
    let mut did = false;
    let read_max = match &pref.borrow().io {
        ProcIo::Datagram(io) => io.read_max,
        _ => return Ok(false),
    };
    let mut bytes = vec![0u8; read_max];
    // Bound each turn so a busy peer cannot starve timers/other processes.
    for _ in 0..64 {
        let received = {
            let mut p = pref.borrow_mut();
            if p.dead || p.status != "open" || matches!(p.filter, Value::Sym(s) if s == sym::T) {
                break;
            }
            let ProcIo::Datagram(io) = &mut p.io else {
                break;
            };
            match io.recv(&mut bytes) {
                Ok((n, remote)) => {
                    io.remote = Some(remote);
                    Ok(n)
                }
                Err(e) => Err(e),
            }
        };
        match received {
            Ok(n) if n > 0 => {
                did = true;
                deliver_output(i, pref, bytes[..n].to_vec(), false)?;
            }
            Ok(0) if pref.borrow().codecs.output.has_pending() =>
            {
                // GNU first flushes an incomplete character as raw bytes. That
                // read produced output; a later empty datagram closes it.
                let buffer = pref.borrow().buffer;
                let value = decode_process_output(i, pref, &[], false, true, buffer)?;
                did = true;
                deliver_decoded(i, pref, value)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            // GNU 31.1 treats a zero-length received datagram like a closed
            // network process, and deactivates it on permanent read errors.
            _ => {
                {
                    let mut p = pref.borrow_mut();
                    p.io = ProcIo::None;
                    p.status = "closed";
                    p.exit_status = 256;
                    p.reported = true;
                }
                if let Some(pos) = i.processes.iter().position(|p| Rc::ptr_eq(p, pref)) {
                    i.processes.remove(pos);
                }
                run_sentinel(i, pref, "closed", 256)?;
                break;
            }
        }
    }
    Ok(did)
}

/// Poll all live processes once; returns true if anything happened.
pub(crate) fn poll_all(i: &mut Interp) -> Result<bool, Flow> {
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

/// Use the first available suffix, considering every registered process.
fn unique_process_name(i: &Interp, requested: &str) -> String {
    let used = |name: &str| i.processes.iter().any(|p| p.borrow().name == name);
    if !used(requested) {
        return requested.to_string();
    }
    for suffix in 1usize.. {
        let candidate = format!("{requested}<{suffix}>");
        if !used(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

fn free_process_marker() -> MarkerRef {
    crate::lisp::memory::allocate(
        Marker {
            buffer: None,
            position: 0,
            insertion_type: false,
        },
        crate::lisp::Value::Marker,
    )
}

/// Associate a process mark only when its new non-nil buffer differs.
/// Detaching the process buffer does not detach or replace the marker.
fn update_process_marker(i: &Interp, p: &mut Proc) {
    let mark = p.mark.get_or_insert_with(free_process_marker).clone();
    let Some(bid) = p.buffer else { return };
    if mark.borrow().buffer == Some(bid) {
        return;
    }
    let buffer = i.buffers.get(bid).filter(|b| b.borrow().live);
    {
        let mut m = mark.borrow_mut();
        m.buffer = buffer.as_ref().map(|_| bid);
        m.position = buffer.as_ref().map(|b| b.borrow().text_len()).unwrap_or(0);
    }
    if let Some(buffer) = buffer {
        buffer.borrow_mut().register_marker(&mark);
    }
}

fn base_proc(name: String, kind: &'static str, io: ProcIo) -> Proc {
    Proc {
        name,
        kind,
        command: Value::Nil,
        io,
        pid: 0,
        status: "run",
        exit_status: 0,
        connect_error: Value::Nil,
        buffer: None,
        mark: Some(free_process_marker()),
        filter: Value::Nil,
        sentinel: Value::Nil,
        plist: Value::Nil,
        query_on_exit: true,
        kill_without_query: false,
        tty_name: Value::Nil,
        coding: ("utf-8-unix".into(), "utf-8-unix".into()),
        codecs: Default::default(),
        stderr_dest: Value::Nil,
        inherit_coding: false,
        connection_type: Value::Nil,
        contact: Value::Nil,
        dead: false,
        reported: false,
        opening: false,
        start_stopped: false,
        pending_status: None,
        gnutls: None,
        gnutls_initstage: 0,
        gnutls_log_level: 0,
        gnutls_async_connected: Value::Nil,
        gnutls_async_signalled: Value::Nil,
    }
}

/// Shared :buffer/:filter/:sentinel/etc keyword handling.
fn finish_setup(
    i: &mut Interp,
    mut p: Proc,
    args: &Value,
    want_command: bool,
) -> Result<ProcessRef, Flow> {
    p.coding = coding::constructor(i, args, p.kind == "pipe")?;
    let b = kw(i, args, ":buffer");
    if b.truthy() {
        // GNU creates the named buffer when it doesn't exist.
        let bid = match i.buffer_id_of(&b) {
            Some(id) => id,
            None => match &b {
                Value::Str(s) => i.create_buffer(&s.borrow()),
                _ => return Err(i.error("No such buffer")),
            },
        };
        p.buffer = Some(bid);
    }
    update_process_marker(i, &mut p);
    p.filter = kw(i, args, ":filter");
    p.sentinel = kw(i, args, ":sentinel");
    if p.sentinel.is_nil() {
        p.sentinel = symv(i, "internal-default-process-sentinel");
    }
    // GNU ignores :plist on `make-process' (child procs) but applies it
    // for pipe, network, and serial processes.
    if !want_command {
        p.plist = kw(i, args, ":plist");
    }
    if kw(i, args, ":noquery").truthy() {
        p.query_on_exit = false;
    }
    if kw(i, args, ":stop").truthy() {
        p.start_stopped = true;
        p.pending_status = Some(p.status);
        p.status = "stop";
        if p.kind == "network" {
            p.command = Value::t();
        }
    }
    let ct = kw(i, args, ":connection-type");
    if ct.truthy() {
        p.connection_type = ct;
    }
    let sd = kw(i, args, ":stderr");
    if sd.truthy() {
        p.stderr_dest = sd;
    }
    if want_command {
        p.command = kw(i, args, ":command");
    }
    p.name = unique_process_name(i, &p.name);
    Ok(crate::lisp::memory::allocate(
        p,
        crate::lisp::Value::Process,
    ))
}



/// GNU is_pty_from_symbol: nil inherits the dynamic default on either side.
fn child_pty_mode(i: &mut Interp, mode: &Value) -> Result<bool, Flow> {
    if mode.is_nil() {
        let variable = i.intern("process-connection-type");
        return Ok(i.symbol_value(variable).truthy());
    }
    if let Value::Sym(symbol) = mode {
        if *symbol == i.intern("pty") {
            return Ok(true);
        }
        if *symbol == i.intern("pipe") {
            return Ok(false);
        }
    }
    Err(i.signal_data(
        sym::FILE_MISSING,
        vec![
            Value::string("Unknown connection type"),
            Value::string("No such file or directory"),
            mode.clone(),
        ],
    ))
}

fn f_make_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_process: nargs==0 returns nil before any checking.
    if a.is_empty() {
        return Ok(Value::Nil);
    }
    if a.len() % 2 != 0 {
        let error = i.intern("malformed-keyword-arg-list");
        return Err(i.signal_data(error, vec![]));
    }
    let name_key = i.intern(":name");
    if !a
        .as_slice()
        .chunks_exact(2)
        .any(|pair| matches!(&pair[0], Value::Sym(id) if *id == name_key))
    {
        return Err(i.error("Missing :name keyword parameter"));
    }
    let args = Value::list(a.as_slice().to_vec());
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.error(":name value not a string")),
    };
    // GNU quirk: a non-nil :stop on make-process signals
    // (wrong-type-argument null <value>); pipe processes accept :stop.
    let stop_v = kw(i, &args, ":stop");
    if stop_v.truthy() {
        let pred = i.intern("null");
        let wta = i.intern("wrong-type-argument");
        return Err(i.signal_data(wta, vec![Value::Sym(pred), stop_v]));
    }
    let ct_v = kw(i, &args, ":connection-type");
    let (pty_input, pty_output) = if let Value::Cons(pair) = &ct_v {
        let (input, output) = {
            let pair = pair.borrow();
            (pair.car.clone(), pair.cdr.clone())
        };
        (child_pty_mode(i, &input)?, child_pty_mode(i, &output)?)
    } else {
        let pty = child_pty_mode(i, &ct_v)?;
        (pty, pty)
    };
    let command = kw(i, &args, ":command");
    let cmd: Vec<String> = match &command {
        // GNU: non-list :command → (wrong-type-argument listp VALUE),
        // not a generic error; nil stays a no-command process.
        Value::Cons(_) | Value::Nil => {
            let items = command
                .list_to_vec()
                .map_err(|_| i.wrong_type_mut("listp", &command))?;
            let mut v = Vec::new();
            for it in items {
                match &it {
                    Value::Str(s) => v.push(s.borrow().clone()),
                    // GNU CHECK_STRINGs each element.
                    _ => return Err(i.wrong_type_mut("stringp", &it)),
                }
            }
            v
        }
        _ => return Err(i.wrong_type_mut("listp", &command)),
    };
    coding::constructor(i, &args, false)?;
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
    let stderr_coding = if split_stderr && !matches!(stderr_dest, Value::Process(_)) {
        coding::constructor(i, &Value::Nil, true)?.0
    } else { String::new() };
    // Transfer an explicitly supplied pipe's writer to the child, as GNU
    // does. Other :stderr destinations are drained through a separate pipe.
    let stderr_pipe = if let Value::Process(sp) = &stderr_dest {
        let mut sp = sp.borrow_mut();
        if let ProcIo::Pipe { child_wfd, .. } = &mut sp.io {
            if *child_wfd >= 0 {
                let fd = std::mem::replace(child_wfd, -1);
                // SAFETY: the pipe relinquishes its sole ownership here.
                Some(unsafe { OwnedFd::from_raw_fd(fd) })
            } else {
                None
            }
        } else {
            unreachable!()
        }
    } else {
        None
    };
    let (io, tty_name) = match child::spawn(&cmd, pty_input, pty_output, stderr_pipe, split_stderr)
    {
        Ok(transport) => transport,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(i.signal_data(
                sym::FILE_MISSING,
                vec![
                    Value::string("Searching for program"),
                    Value::string("No such file or directory"),
                    Value::string(cmd[0].clone()),
                ],
            ));
        }
        Err(e) => return Err(i.error(format!("Process not started: {e}"))),
    };
    let pid = match &io {
        ProcIo::Child { child, .. } => child.id() as i32,
        _ => unreachable!(),
    };
    let mut p = base_proc(name, "real", io);
    p.tty_name = tty_name.map(Value::string).unwrap_or(Value::Nil);
    p.codecs.stderr_name = stderr_coding;
    p.pid = pid;
    let pref = finish_setup(i, p, &args, true)?;
    i.processes.push(pref.clone());
    if pref.borrow().start_stopped {
        unsafe { libc::kill(pid, libc::SIGSTOP) };
    }
    Ok(Value::Process(pref))
}

fn f_make_pipe_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let args = Value::list(a.as_slice().to_vec());
    let name_v = kw(i, &args, ":name");
    // GNU Fmake_pipe_process: `(make-pipe-process)' with no plist returns
    // nil quietly; an explicit :name-less-but-nonempty plist still needs
    // :name? — observed: bare () → nil.
    if a.is_empty() {
        return Ok(Value::Nil);
    }
    let name = match &name_v {
        // GNU Fmake_pipe_process: non-string non-nil :name → ":name value
        // not a string"; missing :name → "Missing :name keyword
        // parameter".
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => return Err(i.error("Missing :name keyword parameter")),
        _ => return Err(i.error(":name value not a string")),
    };
    coding::constructor(i, &args, true)?;
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
    let mut p = base_proc(
        name,
        "pipe",
        ProcIo::Pipe {
            read_fd: fds[0],
            child_wfd: fds[1],
            sink_fd: wfds[1],
        },
    );
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
        // GNU accepts integers or strings; anything else gets a
        // stringp type error.
        _ => Err(i.wrong_type_mut("stringp", v)),
    }
}

/// GNU leaves UDP clients unbound/unconnected until the first sendto. Using
/// UdpSocket::bind here would allocate a port early and misreport :local.
fn unbound_udp(ipv6: bool) -> std::io::Result<std::net::UdpSocket> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let fd = unsafe {
        libc::socket(
            if ipv6 { libc::AF_INET6 } else { libc::AF_INET },
            libc::SOCK_DGRAM,
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: socket returned a new owned UDP descriptor. From this point the
    // standard-library socket closes it on every failure path.
    let socket = unsafe { std::net::UdpSocket::from_raw_fd(fd) };
    if unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(socket)
}

fn datagram_error(i: &mut Interp, operation: &str, error: std::io::Error) -> Flow {
    let symbol = i.intern(if error.kind() == std::io::ErrorKind::PermissionDenied {
        "permission-denied"
    } else {
        "file-error"
    });
    let message = error.to_string();
    i.signal_data(
        symbol,
        vec![
            Value::string(operation),
            Value::string(message.split(" (os error").next().unwrap_or(&message)),
        ],
    )
}

fn make_datagram_process(i: &mut Interp, name: String, args: &Value) -> EvalResult {
    use std::net::ToSocketAddrs;
    let server = kw(i, args, ":server").truthy();
    if server && kw(i, args, ":nowait").truthy() {
        return Err(i.error_q("`:server' is incompatible with `:nowait'", &[]));
    }
    let explicit = kw(i, args, if server { ":local" } else { ":remote" });
    let address = if explicit.truthy() {
        DatagramAddress::from_value(&explicit).ok_or_else(|| i.error("Malformed address"))?
    } else {
        let family = kw(i, args, ":family");
        let family = match &family {
            Value::Nil => None,
            Value::Sym(s) => Some(i.symbol_name(*s)),
            _ => return Err(i.error("Unsupported address family")),
        };
        let service = kw(i, args, ":service");
        if matches!(family.as_deref(), Some("local" | "unix")) {
            DatagramAddress::Local(
                str_value(&service).ok_or_else(|| i.wrong_type_mut("stringp", &service))?,
            )
        } else {
            if !matches!(family.as_deref(), None | Some("ipv4" | "ipv6")) {
                return Err(i.error("Unsupported address family"));
            }
            let port = if server && matches!(service, Value::Sym(s) if s == sym::T) {
                0
            } else {
                service_port(i, &service)?
            };
            let host = kw(i, args, ":host");
            let host = match host {
                Value::Nil => {
                    if family.as_deref() == Some("ipv6") {
                        "::1".into()
                    } else {
                        "127.0.0.1".into()
                    }
                }
                Value::Str(s) => s.borrow().clone(),
                other => return Err(i.wrong_type_mut("stringp", &other)),
            };
            let address = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(|e| datagram_error(i, "make network process failed", e))?
                .find(|addr| match family.as_deref() {
                    Some("ipv4") => addr.is_ipv4(),
                    Some("ipv6") => addr.is_ipv6(),
                    _ => true,
                })
                .ok_or_else(|| i.error("No address in requested family"))?;
            DatagramAddress::Inet(address)
        }
    };
    let operation = if server {
        "Cannot bind server socket"
    } else {
        "make client process failed"
    };
    let socket = match &address {
        DatagramAddress::Inet(addr) => DatagramSocket::Inet(
            if server {
                std::net::UdpSocket::bind(addr)
            } else {
                unbound_udp(addr.is_ipv6())
            }
            .map_err(|e| datagram_error(i, operation, e))?,
        ),
        DatagramAddress::Local(path) => DatagramSocket::Local(
            if server {
                std::os::unix::net::UnixDatagram::bind(path)
            } else {
                std::os::unix::net::UnixDatagram::unbound()
            }
            .map_err(|e| datagram_error(i, operation, e))?,
        ),
    };
    let remote = if server {
        DatagramAddress::from_value(&kw(i, args, ":remote")).filter(|a| a.size() == address.size())
    } else {
        Some(address.clone())
    };
    let io = DatagramIo {
        socket,
        remote,
        address_size: address.size(),
        read_max: {
            let id = i.intern("read-process-output-max");
            // IP datagrams cannot exceed 65535 bytes. Larger configured
            // buffers do not needlessly allocate unbounded memory per poll.
            let requested = i
                .symbol_value(id)
                .int()
                .unwrap_or(65536)
                .clamp(1, i32::MAX as i128) as usize;
            if matches!(address, DatagramAddress::Inet(_)) {
                requested.min(65536)
            } else {
                requested
            }
        },
        bound_path: if server {
            match &address {
                DatagramAddress::Local(p) => Some(p.clone()),
                _ => None,
            }
        } else {
            None
        },
        pending: VecDeque::new(),
    };
    let local = match &io.socket {
        DatagramSocket::Inet(s) => {
            s.set_nonblocking(true)
                .map_err(|e| datagram_error(i, operation, e))?;
            socket_addr_value(
                s.local_addr()
                    .map_err(|e| datagram_error(i, operation, e))?,
            )
        }
        DatagramSocket::Local(s) => {
            s.set_nonblocking(true)
                .map_err(|e| datagram_error(i, operation, e))?;
            s.local_addr()
                .map(|a| {
                    Value::string(
                        a.as_pathname()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )
                })
                .map_err(|e| datagram_error(i, operation, e))?
        }
    };
    let mut contact = args.clone();
    // Only numeric IP services are resolved; Unix services remain pathnames.
    if server && let Value::Vec(parts) = &local {
        contact = super::eval::plist_put(
            &contact,
            i.intern(":service"),
            parts.borrow().last().unwrap().clone(),
        );
    }
    contact = super::eval::plist_put(&contact, i.intern(":local"), local);
    if !server {
        contact = super::eval::plist_put(&contact, i.intern(":remote"), address.value());
    }
    let mut p = base_proc(name, "network", ProcIo::Datagram(io));
    p.status = "open";
    p.contact = contact;
    let pref = finish_setup(i, p, args, false)?;
    i.processes.push(pref.clone());
    Ok(Value::Process(pref))
}

/// One sendto is one message. A hypothetical short datagram write must never
/// be retried as a second packet containing the suffix.
fn send_whole_datagram(
    bytes: &[u8],
    send: impl FnOnce(&[u8]) -> std::io::Result<usize>,
) -> std::io::Result<()> {
    match send(bytes)? {
        n if n == bytes.len() => Ok(()),
        _ => Err(std::io::ErrorKind::WriteZero.into()),
    }
}

fn write_datagram(i: &mut Interp, p: &ProcessRef, bytes: &[u8]) -> EvalResult {
    {
        let mut pb = p.borrow_mut();
        if pb.dead || !pb.alive() {
            return Err(i.error("Process is not running"));
        }
        // GNU process-send-string/process-send-region with zero bytes is a
        // no-op. An actual received empty packet has different semantics.
        if bytes.is_empty() {
            return Ok(Value::Nil);
        }
        let ProcIo::Datagram(io) = &mut pb.io else {
            unreachable!()
        };
        io.pending.push_back(bytes.to_vec());
    }
    loop {
        if i.quit_flag {
            i.quit_flag = false;
            return Err(Flow::Quit);
        }
        let result = {
            let mut pb = p.borrow_mut();
            if pb.dead || !pb.alive() {
                return Err(i.error("Process is not running"));
            }
            let ProcIo::Datagram(io) = &mut pb.io else {
                return Err(i.error("Process is not running"));
            };
            let Some(bytes) = io.pending.front() else {
                return Ok(Value::Nil);
            };
            let result = send_whole_datagram(bytes, |bytes| io.send(bytes, &io.remote));
            if result.is_ok()
                || result.as_ref().is_err_and(|e| {
                    !matches!(
                        e.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    )
                })
            {
                io.pending.pop_front();
            }
            result
        };
        match result {
            Ok(()) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Release the process borrow before Lisp, as with child input.
                // Nested sends append whole messages behind the pending one.
                timer_check(i)?;
                poll_all(i)?;
                crate::lisp::eval::threads::pause(i, std::time::Duration::from_millis(5))?;
            }
            Err(e) => {
                let symbol = i.intern("file-error");
                let message = e.to_string();
                return Err(i.signal_data(
                    symbol,
                    vec![
                        Value::string(if e.raw_os_error() == Some(libc::EMSGSIZE) {
                            "Sending datagram"
                        } else {
                            "Writing to process"
                        }),
                        Value::string(message.split(" (os error").next().unwrap_or(&message)),
                        Value::Process(p.clone()),
                    ],
                ));
            }
        }
    }
}

fn f_make_network_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let args = Value::list(a.as_slice().to_vec());
    let name_v = kw(i, &args, ":name");
    let name = match &name_v {
        Value::Str(s) => s.borrow().clone(),
        Value::Sym(id) if name_v.truthy() => i.symbol_name(*id),
        _ => return Err(i.error("Network process name not given")),
    };
    coding::constructor(i, &args, false)?;
    let type_v = kw(i, &args, ":type");
    if matches!(&type_v, Value::Sym(id) if i.symbol_name(*id) == "datagram") {
        return make_datagram_process(i, name, &args);
    }
    if type_v.truthy() {
        return Err(i.error("Unsupported connection type"));
    }
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
    // `:family local' — Unix-domain sockets; :service is the socket path.
    let family_v = kw(i, &args, ":family");
    let is_local = matches!(&family_v, Value::Sym(s) if {
        let n = i.symbol_name(*s);
        n == "local" || n == "unix"
    });
    if is_local {
        let path = match &service {
            Value::Str(s) => s.borrow().clone(),
            _ => return Err(i.wrong_type_mut("stringp", &service)),
        };
        if server {
            if kw(i, &args, ":nowait").truthy() {
                return Err(i.error_q("`:server' is incompatible with `:nowait'", &[]));
            }
            let listener = std::os::unix::net::UnixListener::bind(&path).map_err(|e| {
                let sig = i.intern(if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "permission-denied"
                } else {
                    "file-error"
                });
                let msg = e.to_string();
                let msg = msg.split(" (os error").next().unwrap_or(&msg).to_string();
                i.signal_data(
                    sig,
                    vec![
                        Value::string("Cannot bind server socket"),
                        Value::string(msg),
                    ],
                )
            })?;
            let _ = listener.set_nonblocking(true);
            let mut p = base_proc(name, "network", ProcIo::LocalListen(listener, path.clone()));
            p.status = "listen";
            let local_kw = symv(i, ":local");
            p.contact = {
                let mut items: Vec<Value> = args.list_to_vec().unwrap_or_default();
                items.extend(vec![local_kw, Value::string(path.clone())]);
                Value::list(items)
            };
            let pref = finish_setup(i, p, &args, false)?;
            i.processes.push(pref.clone());
            return Ok(Value::Process(pref));
        }
        let stream = match std::os::unix::net::UnixStream::connect(&path) {
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
        let mut p = base_proc(name, "network", ProcIo::LocalNet(stream));
        p.status = "open";
        let local_kw = symv(i, ":local");
        p.contact = {
            let mut items: Vec<Value> = args.list_to_vec().unwrap_or_default();
            items.extend(vec![local_kw, Value::string(path)]);
            Value::list(items)
        };
        let pref = finish_setup(i, p, &args, false)?;
        i.processes.push(pref.clone());
        return Ok(Value::Process(pref));
    }
    if server {
        if kw(i, &args, ":nowait").truthy() {
            return Err(i.error_q("`:server' is incompatible with `:nowait'", &[]));
        }
        // GNU's :service t asks the kernel to choose an ephemeral port for
        // an Internet server, just like port zero. This server-only case
        // does not change client parsing or local-domain socket paths.
        let port = if matches!(service, Value::Sym(s) if s == sym::T) {
            0
        } else {
            service_port(i, &service)?
        };
        // An address family does not replace the requested host.  In
        // particular, :family ipv4 must not broaden a loopback bind to
        // every interface (issue #232).
        let listener = std::net::TcpListener::bind((host.as_str(), port)).map_err(|e| {
            // GNU reports bind failures via the errno-derived condition:
            // EACCES → permission-denied, others → file-error, with data
            // ("Cannot bind server socket" <strerror>).
            let sig = i.intern(if e.kind() == std::io::ErrorKind::PermissionDenied {
                "permission-denied"
            } else {
                "file-error"
            });
            let msg = e.to_string();
            let msg = msg.split(" (os error").next().unwrap_or(&msg).to_string();
            i.signal_data(
                sig,
                vec![
                    Value::string("Cannot bind server socket"),
                    Value::string(msg),
                ],
            )
        })?;
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
    let nowait = kw(i, &args, ":nowait").truthy();
    let connection = if nowait {
        use std::net::ToSocketAddrs;
        // Name resolution follows the platform's resolver; no TCP handshake
        // is allowed to block, including fallback to another resolved address.
        (host.as_str(), port)
            .to_socket_addrs()
            .and_then(|addresses| {
                let family = match family_v {
                    Value::Sym(s) => i.symbol_name(s),
                    _ => String::new(),
                };
                TcpConnect::start(
                    addresses
                        .filter(|addr| match family.as_str() {
                            "ipv4" => addr.is_ipv4(),
                            "ipv6" => addr.is_ipv6(),
                            _ => true,
                        })
                        .collect(),
                )
                .map(ProcIo::Connecting)
            })
    } else {
        std::net::TcpStream::connect((host.as_str(), port)).and_then(|stream| {
            stream.set_nonblocking(true)?;
            Ok(ProcIo::Net(stream))
        })
    };
    let io = match connection {
        Ok(io) => io,
        Err(e) => {
            // GNU: (file-error "make client process failed" <errno> <plist>)
            let fe = i.intern("file-error");
            if nowait {
                let message = e.to_string();
                let mut data = vec![
                    Value::Sym(fe),
                    Value::string("make client process failed"),
                    Value::string(message.split(" (os error").next().unwrap_or(&message)),
                ];
                data.extend(args.list_to_vec().unwrap_or_default());
                let mut p = base_proc(name, "network", ProcIo::None);
                p.status = "failed";
                p.exit_status = e.raw_os_error().unwrap_or(libc::EIO);
                p.connect_error = Value::list(data);
                p.contact = args.clone();
                // No readiness notification exists in this immediate-error
                // case. Explicit deletion still delivers a deleted event.
                p.reported = true;
                let pref = finish_setup(i, p, &args, false)?;
                i.processes.push(pref.clone());
                return Ok(Value::Process(pref));
            }
            let mut data = vec![
                Value::string("make client process failed"),
                Value::string(e.to_string()),
            ];
            data.extend(args.list_to_vec().unwrap_or_default());
            return Err(i.signal_data(fe, data));
        }
    };
    let (remote, local) = match &io {
        ProcIo::Connecting(connect) => (
            socket_addr_value(connect.remote),
            connect
                .socket
                .as_ref()
                .and_then(|s| s.local_addr().ok())
                .map(socket_addr_value)
                .unwrap_or(Value::Nil),
        ),
        ProcIo::Net(stream) => (
            addr_vec(&host, port),
            addr_vec(
                "127.0.0.1",
                stream.local_addr().map(|a| a.port()).unwrap_or(0),
            ),
        ),
        _ => unreachable!("TCP client backend"),
    };
    let mut p = base_proc(name, "network", io);
    // Even an immediately successful nonblocking connect is observable as
    // `connect' until an event-loop turn delivers its open sentinel.
    p.status = if nowait { "connect" } else { "open" };
    let remote_kw = symv(i, ":remote");
    let local_kw = symv(i, ":local");
    p.contact = contact_plist(i, port, vec![remote_kw, remote, local_kw, local]);
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
    Value::vector(octets)
}

fn f_make_serial_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let args = Value::list(a.as_slice().to_vec());
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
    coding::constructor(i, &args, false)?;
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

fn f_serial_process_configure(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: odd-length
    // keyword list → malformed-keyword-arg-list before anything else.
    if a.len() % 2 != 0
        || a.as_slice()
            .chunks_exact(2)
            .any(|kv| matches!(&kv[1], Value::Sym(s) if i.symbol_name(*s).starts_with(':')))
    {
        let s = i.intern("malformed-keyword-arg-list");
        return Err(i.signal_data(s, vec![Value::Nil]));
    }
    let args = Value::list(a.as_slice().to_vec());
    let pv = kw(i, &args, ":process");
    let pref = match pv {
        // nil :process = the current buffer's process.
        Value::Nil => {
            // nil :process = the current buffer's process, resolved the
            // same way f_get_buffer_process does (live proc whose
            // `buffer' field is the current buffer).
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
                    return Err(i.error(&format!("Buffer {} has no process", name)));
                }
            }
        }
        Value::Process(p) => p,
        other => return Err(i.wrong_type_mut("processp", &other)),
    };
    if let Value::Int(speed) = kw(i, &args, ":speed") {
        serial_set_speed(&pref, speed as u32);
    }
    Ok(Value::Nil)
}

fn f_start_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (name buffer program &rest args)
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        // GNU: passes NAME through to make-process, whose
        // get_required_string_keyword_param rejects non-strings
        // (incl. symbols) with (error ":name value not a string").
        _ => return Err(i.error(":name value not a string")),
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
    f_make_process(i, plist_items.into())
}

// ---------- predicates / accessors ----------

fn f_processp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Process(_))))
}

fn f_get_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fget_process: CHECK_STRING — nil and buffers signal
    // stringp; names just look up the process alist (nil when missing).
    match proc_by_name(i, &a[0])? {
        Some(p) => Ok(Value::Process(p)),
        None => Ok(Value::Nil),
    }
}

fn f_process_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_list_processes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU displays a "*Process List*" buffer and returns nil; when called
    // with QUERY-ONLY it filters. Keep the display minimal: create the
    // buffer and return nil like GNU.
    let _ = a;
    let bid = i.create_buffer("*Process List*");
    let mut lines = String::from(
        "Proc         Status   Buffer         Tty         Command\n----         ------   ------         ---         -------\n",
    );
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
    if i.buffers.get(bid).is_some() {
        let _ = crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
            crate::buffer::primitives::chg_insert(i, 0, &lines)?;
            crate::buffer::primitives::cur(i).borrow_mut().point = 0;
            Ok(Value::Nil)
        });
    }
    Ok(Value::Nil)
}

fn f_process_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::string(p.borrow().name.clone()))
}

fn f_process_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let bid = p.borrow().buffer;
    Ok(bid.and_then(|b| i.buffer_value(b)).unwrap_or(Value::Nil))
}

fn f_set_process_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let bid = match &a[1] {
        Value::Nil => None,
        Value::Buffer(b) => Some(b.borrow().id),
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    let mut pb = p.borrow_mut();
    if pb.buffer != bid {
        pb.buffer = bid;
        update_process_marker(i, &mut pb);
    }
    Ok(a[1].clone())
}

fn f_process_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().command.clone())
}

fn f_process_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fprocess_type decodes through `get_process' — names and
    // buffers resolve, unlike the CHECK_PROCESS accessors.
    let p = get_process(i, &a[0])?;
    let k = p.borrow().kind;
    Ok(symv(i, k))
}

fn f_process_id(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::Int(p.borrow().pid as i128))
}

fn f_process_status(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU reads the cached status — updated only by status notification
    // during waits (accept-process-output et al), not by this query.
    // A string is a quiet Fget_process lookup (nil when unknown); nil
    // and buffers go through `get_process' and can signal.
    let p = match &a[0] {
        Value::Str(_) => match proc_by_name(i, &a[0])? {
            Some(p) => p,
            None => return Ok(Value::Nil),
        },
        _ => get_process(i, &a[0])?,
    };
    Ok(symv(i, p.borrow().status))
}

fn f_process_exit_status(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    if p.borrow().connect_error.truthy() {
        return Ok(p.borrow().connect_error.clone());
    }
    Ok(Value::Int(p.borrow().exit_status as i128))
}

fn f_process_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let mark = p
        .borrow_mut()
        .mark
        .get_or_insert_with(free_process_marker)
        .clone();
    Ok(Value::Marker(mark))
}

fn f_process_contact(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (mut c, kind) = {
        let pb = p.borrow();
        (pb.contact.clone(), pb.kind)
    };
    let key = arg(&a, 1);
    if matches!(&key, Value::Sym(id) if *id == sym::T || i.symbol_name(*id) == ":remote") {
        if let ProcIo::Datagram(io) = &p.borrow().io {
            c = super::eval::plist_put(&c, i.intern(":remote"), io.remote_value());
        }
    }
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

fn f_process_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().plist.clone())
}

fn f_set_process_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().plist = a[1].clone();
    Ok(a[1].clone())
}

fn f_process_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let prop = match &a[1] {
        Value::Sym(id) => *id,
        _ => return Err(i.wrong_type_mut("symbolp", &a[1])),
    };
    Ok(plist_get(&p.borrow().plist, prop))
}

fn f_process_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_process_query_on_exit_flag(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::from_bool(p.borrow().query_on_exit))
}

fn f_set_process_query_on_exit_flag(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().query_on_exit = a[1].truthy();
    Ok(a[1].clone())
}

fn f_process_filter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let f = p.borrow().filter.clone();
    if f.truthy() {
        Ok(f)
    } else {
        Ok(symv(i, "internal-default-process-filter"))
    }
}

fn f_set_process_filter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().filter = a[1].clone();
    Ok(a[1].clone())
}

fn f_process_sentinel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(p.borrow().sentinel.clone())
}

fn f_set_process_sentinel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let sentinel = if a[1].is_nil() {
        symv(i, "internal-default-process-sentinel")
    } else {
        a[1].clone()
    };
    p.borrow_mut().sentinel = sentinel.clone();
    Ok(sentinel)
}

fn f_process_tty_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let stream = arg(&a, 1);
    let p = p.borrow();
    let uses_tty = if stream.is_nil() {
        true
    } else {
        let stdin = i.intern("stdin");
        let stdout = i.intern("stdout");
        let stderr = i.intern("stderr");
        match &stream {
            Value::Sym(id) if *id == stdin => matches!(
                &p.io,
                ProcIo::Child {
                    pty_input: true,
                    ..
                }
            ),
            Value::Sym(id) if *id == stdout => matches!(
                &p.io,
                ProcIo::Child {
                    pty_output: true,
                    ..
                }
            ),
            Value::Sym(id) if *id == stderr => {
                matches!(
                    &p.io,
                    ProcIo::Child {
                        pty_output: true,
                        ..
                    }
                ) && p.stderr_dest.is_nil()
            }
            _ => {
                return Err(i.signal_data(sym::ERROR, vec![Value::string("Unknown stream"), stream]));
            }
        }
    };
    Ok(if uses_tty {
        p.tty_name.clone()
    } else {
        Value::Nil
    })
}

fn f_process_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (d, e) = p.borrow().coding.clone();
    Ok(Value::cons(symv(i, &d), symv(i, &e)))
}

fn f_set_process_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let read = arg(&a, 1);
    let write = arg(&a, 2);
    let read_name = coding::name(i, &read)?;
    let write_name = coding::encoding_name(i, &write)?;
    p.borrow_mut().coding = (read_name, write_name);
    Ok(Value::Nil)
}

fn f_process_inherit_coding_system_flag(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(Value::from_bool(p.borrow().inherit_coding))
}

fn f_set_process_inherit_coding_system_flag(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    p.borrow_mut().inherit_coding = a[1].truthy();
    Ok(a[1].clone())
}

fn f_set_process_window_size(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_process_datagram_address(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    Ok(match &p.borrow().io {
        ProcIo::Datagram(io) => io.remote_value(),
        _ => Value::Nil,
    })
}

fn f_set_process_datagram_address(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    if let ProcIo::Datagram(io) = &mut p.borrow_mut().io {
        if let Some(remote) = DatagramAddress::from_value(&a[1]) {
            if remote.size() == io.address_size {
                io.remote = Some(remote);
                return Ok(a[1].clone());
            }
        }
    }
    Ok(Value::Nil)
}

fn f_get_buffer_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bid = match &a[0] {
        Value::Nil => Some(i.current_buffer),
        // GNU goes through `Fget_buffer': non-string non-buffer args
        // signal stringp; a name with no live buffer yields nil.
        v @ (Value::Str(_) | Value::Buffer(_)) => i.buffer_id_of(v),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let Some(bid) = bid else {
        return Ok(Value::Nil);
    };
    for p in &i.processes {
        let pb = p.borrow();
        if !pb.dead && pb.buffer == Some(bid) {
            return Ok(Value::Process(p.clone()));
        }
    }
    Ok(Value::Nil)
}

// ---------- I/O ----------

fn f_accept_process_output(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (process &optional seconds millisec just-this-one)
    let target = match a.first().filter(|v| v.truthy()) {
        Some(v) => Some(want_proc(i, v)?),
        None => None,
    };
    // GNU accepts any number for SECONDS (fractional waits), a fixnum
    // for MILLISEC.
    let secs = match a.get(1) {
        None | Some(Value::Nil) => 0.0,
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(f)) => **f,
        Some(v) => {
            let wta = i.intern("wrong-type-argument");
            let fxp = i.intern("numberp");
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
    // PROCESS=nil ignores JUST-THIS-ONE. Otherwise every non-nil value
    // confines reads to PROCESS; GNU additionally inhibits this wait's
    // timers for fixnums (including zero), but not bignums or floats.
    let just_this_one = target.is_some() && a.get(3).is_some_and(Value::truthy);
    let inhibit_timers = just_this_one && a.get(3).is_some_and(Value::fixnump);
    let poll = |i: &mut Interp| match &target {
        Some(p) => {
            let got = poll_proc(i, p)?;
            if !just_this_one {
                // With the default flag, other channels still progress
                // (e.g. a server listener while waiting on its client).
                poll_all(i)?;
            }
            Ok(got)
        }
        None => poll_all(i),
    };
    // A due timer is serviced even when output is already buffered.
    if !inhibit_timers {
        timer_check(i)?;
    }
    let mut got = poll(i)?;
    while !got && std::time::Instant::now() < deadline {
        crate::lisp::eval::threads::pause(i, std::time::Duration::from_millis(5))?;
        if !inhibit_timers {
            timer_check(i)?;
        }
        got = poll(i)?;
        // A dead target can't produce more output — GNU returns early.
        if let Some(p) = &target {
            if !p.borrow().alive() {
                break;
            }
        }
    }
    Ok(Value::from_bool(got))
}

fn f_process_send_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks STRING before decoding PROCESS (CHECK_STRING is
    // arg 1's contract in process.c), then decodes PROCESS through
    // `get_process' — buffers/names resolve like the other control verbs.
    if !matches!(&a[1], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[1]));
    }
    let p = get_process(i, &a[0])?;
    let bytes = encode_process_input(i, &p, a[1].clone())?;
    proc_write(i, &p, &bytes)
}

/// Try one nonblocking write, retaining exactly the unwritten suffix.
/// Keeping this queue on the process lets a filter's nested send append behind
/// the outer send and drain both without duplicating or reordering input.
fn write_pending_input(
    pending: &mut VecDeque<u8>,
    mut write: impl FnMut(&[u8]) -> std::io::Result<usize>,
) -> std::io::Result<bool> {
    if pending.is_empty() {
        return Ok(true);
    }
    let bytes = pending.as_slices().0;
    match write(bytes)? {
        0 => Err(std::io::ErrorKind::WriteZero.into()),
        n => {
            pending.drain(..n);
            let complete = pending.is_empty();
            if complete {
                // Do not retain a large send's peak allocation for the entire
                // lifetime of an otherwise idle child process.
                *pending = VecDeque::new();
            }
            Ok(complete)
        }
    }
}

fn write_child_input(i: &mut Interp, p: &ProcessRef, bytes: &[u8]) -> EvalResult {
    {
        let mut pb = p.borrow_mut();
        if pb.dead || pb.status != "run" {
            return Err(i.error("Process is not running"));
        }
        if let ProcIo::Child { pending_input, .. } = &mut pb.io {
            pending_input.extend(bytes);
        }
    }
    loop {
        if i.quit_flag {
            i.quit_flag = false;
            return Err(Flow::Quit);
        }
        let result = {
            let mut pb = p.borrow_mut();
            // A filter or timer may have deleted/reaped the process while we
            // waited. Never retain its descriptor (or a borrow) across Lisp.
            if pb.dead || pb.status != "run" {
                return Err(i.error("Process is not running"));
            }
            let ProcIo::Child {
                input_fd,
                pending_input,
                ..
            } = &mut pb.io
            else {
                return Err(i.error("Writing to process: broken pipe"));
            };
            let Some(input) = input_fd.as_ref() else {
                // GNU redirects later writes to /dev/null after pipe EOF.
                *pending_input = VecDeque::new();
                return Ok(Value::Nil);
            };
            write_pending_input(pending_input, |rest| {
                let n = unsafe { libc::write(input.as_raw_fd(), rest.as_ptr().cast(), rest.len()) };
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            })
        };
        match result {
            Ok(true) => return Ok(Value::Nil),
            Ok(false) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // The child may be blocked writing output before it can read
                // more input. Pump all output (including separate stderr pipe
                // processes) and timers instead of just waiting for POLLOUT.
                timer_check(i)?;
                poll_all(i)?;
                crate::lisp::eval::threads::pause(i, std::time::Duration::from_millis(5))?;
            }
            Err(_) => return Err(i.error("Writing to process: broken pipe")),
        }
    }
}

fn prepare_process_write(i: &mut Interp, p: &ProcessRef) -> Result<(), Flow> {
    // GNU waits for connection completion while servicing the event loop.
    // Never hold a RefCell borrow across callbacks or fall back to a blocking
    // socket; a timer/sentinel can cancel this wait by deleting the process.
    while matches!(p.borrow().io, ProcIo::Connecting(_)) && !p.borrow().dead {
        if i.quit_flag {
            i.quit_flag = false;
            return Err(Flow::Quit);
        }
        timer_check(i)?;
        poll_all(i)?;
        if matches!(p.borrow().io, ProcIo::Connecting(_)) && !p.borrow().dead {
            crate::lisp::eval::threads::pause(i, std::time::Duration::from_millis(5))?;
        }
    }
    let pb = p.borrow();
    if !pb.dead && (pb.status == "failed" || pb.pending_status == Some("failed")) {
        let code = if pb.connect_error.truthy() {
            i.princ_to_string(&pb.connect_error)
        } else {
            pb.exit_status.to_string()
        };
        return Err(i.error(format!(
            "Process {} not running: failed with code {code}\n",
            pb.name
        )));
    }
    if !pb.alive() {
        if pb.kind == "network" && pb.dead {
            return Err(i.error(format!("Process {} not running: deleted\n", pb.name)));
        }
        return Err(i.error("Process is not running"));
    }
    if matches!(pb.io, ProcIo::Child { .. }) && pb.status != "run" {
        return Err(i.error("Process is not running"));
    }
    Ok(())
}

fn proc_write(i: &mut Interp, p: &ProcessRef, bytes: &[u8]) -> EvalResult {
    prepare_process_write(i, p)?;
    if matches!(p.borrow().io, ProcIo::Datagram(_)) {
        return write_datagram(i, p, bytes);
    }
    if matches!(p.borrow().io, ProcIo::Child { .. }) {
        return write_child_input(i, p, bytes);
    }
    let mut pb = p.borrow_mut();
    // Route through the TLS record layer once gnutls-boot completed
    // (GNU's emacs_gnutls_write).
    if matches!(pb.io, ProcIo::Net(_)) && crate::lisp::builtins::gnutls::tls_ready(&pb) {
        return match crate::lisp::builtins::gnutls::record_write(&pb, bytes) {
            Ok(_) => Ok(Value::Nil),
            Err(()) => Err(i.error("Writing to process: broken pipe")),
        };
    }
    let r = match &mut pb.io {
        ProcIo::Child { .. } => unreachable!("child writes use the pending-input queue"),
        ProcIo::Pipe { sink_fd, .. } => {
            let n = unsafe { libc::write(*sink_fd, bytes.as_ptr() as *const _, bytes.len()) };
            if n < 0 { Err(()) } else { Ok(()) }
        }
        ProcIo::Datagram(_) => unreachable!("datagrams have a message-oriented write path"),
        ProcIo::Connecting(_) => unreachable!("connecting writes wait for completion"),
        ProcIo::Net(s) => s.write_all(bytes).map_err(|_| ()),
        ProcIo::LocalNet(s) => s.write_all(bytes).map_err(|_| ()),
        ProcIo::Serial(f) => f.write_all(bytes).map_err(|_| ()),
        ProcIo::Listen(_) | ProcIo::LocalListen(..) | ProcIo::None => Err(()),
    };
    match r {
        Ok(()) => Ok(Value::Nil),
        Err(()) => Err(i.error("Writing to process: broken pipe")),
    }
}

fn f_process_send_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes PROCESS through `get_process' (buffer names too)
    // BEFORE `validate_region' checks START/END.
    let p = get_process(i, &a[0])?;
    let Some(s) = a[1].int() else {
        return Err(i.wrong_type_mut("integer-or-marker-p", &a[1]));
    };
    let Some(e) = a[2].int() else {
        return Err(i.wrong_type_mut("integer-or-marker-p", &a[2]));
    };
    let bid = i.current_buffer;
    let (lo, hi) = i
        .buffers
        .get(bid)
        .map(|b| {
            let b = b.borrow();
            let start = ((s - 1).max(0) as usize).min(b.text_len());
            let end = ((e - 1).max(0) as usize).min(b.text_len());
            (start.min(end), start.max(end))
        })
        .unwrap_or((0, 0));
    let text = crate::buffer::primitives::f_buffer_substring(
        i,
        vec![Value::Int(lo as i128 + 1), Value::Int(hi as i128 + 1)].into(),
    )?;
    let bytes = encode_process_input(i, &p, text)?;
    proc_write(i, &p, &bytes)
}

fn f_process_send_eof(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: PROCESS nil/omitted → the process of the current buffer.
    let p = get_process(i, &arg(&a, 0))?;
    if matches!(p.borrow().io, ProcIo::Datagram(_)) {
        return Ok(arg(&a, 0));
    }
    if matches!(p.borrow().io, ProcIo::Child { .. }) {
        let pty_input = matches!(
            &p.borrow().io,
            ProcIo::Child {
                pty_input: true,
                ..
            }
        );
        write_child_input(i, &p, if pty_input { b"\x04" } else { b"" })?;
        if !pty_input {
            if let ProcIo::Child { input_fd, .. } = &mut p.borrow_mut().io {
                input_fd.take();
            }
        }
    } else {
        let mut pb = p.borrow_mut();
        match &mut pb.io {
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

/// GNU's `Frun_hook_with_args_until_success': call each function on
/// HOOK-NAME's value with ARGS until one returns non-nil; that value
/// is the result (nil when the hook runs out).
fn run_hook_until_success(i: &mut Interp, hook: &str, args: Vec<Value>) -> EvalResult {
    let id = i.intern(hook);
    let list = if i.bound_p(id) {
        i.symbol_value(id)
    } else {
        Value::Nil
    };
    let mut rest = list;
    while let Value::Cons(c) = &rest {
        let (f, cdr) = {
            let cc = c.borrow();
            (cc.car.clone(), cc.cdr.clone())
        };
        let v = i.apply(&f, args.clone())?;
        if !v.is_nil() {
            return Ok(v);
        }
        rest = cdr;
    }
    Ok(Value::Nil)
}

/// GNU's `abbr_to_signal' (process.c): the case-folded abbreviation
/// (an optional SIG/sig prefix is dropped first) maps to a signal
/// number; a decimal name is the number itself (str2sig).  None on
/// an unrecognized name.
fn abbr_to_signal(i: &Interp, id: SymId) -> Option<i32> {
    let name = i.symbol_name(id);
    let up = name.to_uppercase();
    let bare = up.strip_prefix("SIG").unwrap_or(&up);
    if !bare.is_empty() && bare.bytes().all(|b| b.is_ascii_digit()) {
        return bare.parse::<i32>().ok();
    }
    match bare {
        "HUP" => Some(libc::SIGHUP),
        "INT" => Some(libc::SIGINT),
        "QUIT" => Some(libc::SIGQUIT),
        "ILL" => Some(libc::SIGILL),
        "TRAP" => Some(libc::SIGTRAP),
        "ABRT" => Some(libc::SIGABRT),
        "EMT" => Some(libc::SIGEMT),
        "FPE" => Some(libc::SIGFPE),
        "KILL" => Some(libc::SIGKILL),
        "BUS" => Some(libc::SIGBUS),
        "SEGV" => Some(libc::SIGSEGV),
        "SYS" => Some(libc::SIGSYS),
        "PIPE" => Some(libc::SIGPIPE),
        "ALRM" => Some(libc::SIGALRM),
        "TERM" => Some(libc::SIGTERM),
        "URG" => Some(libc::SIGURG),
        "STOP" => Some(libc::SIGSTOP),
        "TSTP" => Some(libc::SIGTSTP),
        "CONT" => Some(libc::SIGCONT),
        "CHLD" => Some(libc::SIGCHLD),
        "CLD" => Some(libc::SIGCHLD),
        "TTIN" => Some(libc::SIGTTIN),
        "TTOU" => Some(libc::SIGTTOU),
        "IO" => Some(libc::SIGIO),
        "POLL" => Some(libc::SIGIO),
        "XCPU" => Some(libc::SIGXCPU),
        "XFSZ" => Some(libc::SIGXFSZ),
        "VTALRM" => Some(libc::SIGVTALRM),
        "PROF" => Some(libc::SIGPROF),
        "WINCH" => Some(libc::SIGWINCH),
        "INFO" => Some(libc::SIGINFO),
        "USR1" => Some(libc::SIGUSR1),
        "USR2" => Some(libc::SIGUSR2),
        _ => None,
    }
}

/// GNU's `string_to_number' (lread.c), base 10: leading integer part
/// (optional sign), a fraction after '.', an `e'/`E' exponent — plus
/// the GNU reader's `e+INF'/`e+NaN' — followed by junk.  Returns
/// (VALUE, consumed-len); None when the syntax is invalid (no leading
/// digits and not float syntax).
fn string_to_number10(s: &str) -> Option<(Value, usize)> {
    let bytes = s.as_bytes();
    let mut cp = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative || bytes.first() == Some(&b'+') {
        cp += 1;
    }
    let mut lead = false;
    while cp < bytes.len() && bytes[cp].is_ascii_digit() {
        lead = true;
        cp += 1;
    }
    let after_digits = cp;
    if cp < bytes.len() && bytes[cp] == b'.' {
        cp += 1;
    }
    let mut trail = false;
    let mut e_exp = false;
    let mut value = 0.0f64;
    while cp < bytes.len() && bytes[cp].is_ascii_digit() {
        trail = true;
        cp += 1;
    }
    if cp < bytes.len() && (bytes[cp] == b'e' || bytes[cp] == b'E') {
        let ecp = cp;
        cp += 1;
        let eplus = cp < bytes.len() && bytes[cp] == b'+';
        if cp < bytes.len() && (bytes[cp] == b'+' || bytes[cp] == b'-') {
            cp += 1;
        }
        if cp < bytes.len() && bytes[cp].is_ascii_digit() {
            e_exp = true;
            while cp < bytes.len() && bytes[cp].is_ascii_digit() {
                cp += 1;
            }
        } else if eplus && s[cp..].starts_with("INF") {
            e_exp = true;
            cp += 3;
            value = f64::INFINITY;
        } else if eplus && s[cp..].starts_with("NaN") {
            e_exp = true;
            cp += 3;
            value = f64::NAN;
        } else {
            cp = ecp;
        }
    }
    let float_syntax = trail || (lead && e_exp);
    if float_syntax {
        let v = if value != 0.0 {
            value
        } else {
            s[..cp].parse::<f64>().unwrap_or(0.0)
        };
        return Some((Value::float(if negative { -v } else { v }), cp));
    }
    if !lead {
        return None;
    }
    let n: i128 = s[..after_digits]
        .parse()
        .unwrap_or(if negative { i128::MIN } else { i128::MAX });
    Some((Value::Int(n), cp))
}

/// GNU `run-hook-with-args-until-success' over `signal-process-functions'
/// — the default member `internal-default-signal-process' does the work.
fn f_signal_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    run_hook_until_success(i, "signal-process-functions", a.as_slice().to_vec())
}

/// GNU `Finternal_default_signal_process' (process.c): PROCESS is a
/// process/name/pid-string/pid number (an unresolvable string quietly
/// yields nil BEFORE SIGCODE is validated), SIGCODE an integer or a
/// signal-name symbol; the result is kill(2)'s return value.
fn f_internal_default_signal_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let process = match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            match proc_by_name(i, &a[0])? {
                Some(p) => Value::Process(p),
                None => match string_to_number10(&name) {
                    Some((v, len)) if len == name.len() => v,
                    _ => return Ok(Value::Nil),
                },
            }
        }
        Value::Int(_) | Value::Float(_) => a[0].clone(),
        other => Value::Process(get_process(i, other)?),
    };
    // GNU CONS_TO_INTEGER(process, pid_t): only an in-range fixnum or
    // an integral in-range float (or cons-of-integers) is a valid pid;
    // 1.5 or 1e300 signal "Not an in-range integer…".
    let pid: i32 = match &process {
        Value::Int(n) if *n >= i32::MIN as i128 && *n <= i32::MAX as i128 => *n as i32,
        Value::Float(f)
            if **f >= i32::MIN as f64
                && **f < 1.0 + i32::MAX as f64
                && (**f as i64) as f64 == **f =>
        {
            **f as i32
        }
        Value::Int(_) | Value::Float(_) => {
            return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
        }
        Value::Process(p) => {
            let pb = p.borrow();
            let pid = pb.pid;
            if pid <= 0 {
                let name = pb.name.clone();
                return Err(i.error(format!("Cannot signal process {name}")));
            }
            pid
        }
        _ => unreachable!(),
    };
    let signo = match &a[1] {
        // GNU check_integer_range(sigcode, INT_MIN, INT_MAX) — a
        // bignum sigcode falls through to CHECK_SYMBOL instead.
        Value::Int(n) => {
            if *n < i32::MIN as i128 || *n > i32::MAX as i128 {
                let s = i.intern("args-out-of-range");
                return Err(i.signal_data(
                    s,
                    vec![
                        a[1].clone(),
                        Value::Int(i32::MIN as i128),
                        Value::Int(i32::MAX as i128),
                    ],
                ));
            }
            *n as i32
        }
        Value::Sym(id) => match abbr_to_signal(i, *id) {
            Some(n) => n,
            None => {
                let name = i.symbol_name(*id);
                return Err(i.error(format!("Undefined signal name {name}")));
            }
        },
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let r = unsafe { libc::kill(pid, signo) };
    Ok(Value::Int(r as i128))
}

/// GNU's shared `get_process' decoder: a process, a process name, a
/// buffer or buffer name, or nil (current buffer's process).
/// Unknown name → "Process %s does not exist"; a buffer with no
/// process → "Buffer %s has no process" (named by buffer name).
pub(crate) fn get_process(i: &mut Interp, v: &Value) -> Result<ProcessRef, Flow> {
    match v {
        Value::Str(s) => {
            let name = s.borrow().clone();
            if let Some(p) = i
                .processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().name == name)
            {
                return Ok(p.clone());
            }
            if let Some(bid) = i.buffers.by_name(&name) {
                if let Some(p) = i
                    .processes
                    .iter()
                    .find(|p| !p.borrow().dead && p.borrow().buffer == Some(bid))
                {
                    return Ok(p.clone());
                }
                return Err(i.error(format!("Buffer {name} has no process")));
            }
            Err(i.error(format!("Process {name} does not exist")))
        }
        Value::Buffer(b) => {
            let bid = b.borrow().id;
            if let Some(p) = i
                .processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().buffer == Some(bid))
            {
                return Ok(p.clone());
            }
            let name = b.borrow().name.clone();
            Err(i.error(format!("Buffer {name} has no process")))
        }
        Value::Nil => {
            let bid = i.current_buffer;
            if let Some(p) = i
                .processes
                .iter()
                .find(|p| !p.borrow().dead && p.borrow().buffer == Some(bid))
            {
                return Ok(p.clone());
            }
            let name = i
                .buffers
                .get(bid)
                .map(|b| b.borrow().name.clone())
                .unwrap_or_default();
            Err(i.error(format!("Buffer {name} has no process")))
        }
        other => want_proc(i, other),
    }
}

/// GNU's `process_send_signal' (process.c): resolve PROCESS through
/// `get_process', require a live real subprocess ("not a subprocess"
/// before "not active"), then kill(2).  Only the signal counts —
/// CURRENT-GROUP merely selects the pgrp in GNU; with `lambda' GNU
/// may skip a shell-owned tty, which our pgrp-less spawn
/// approximates.  Returns PROCESS (the argument, not the object).
fn process_send_signal(i: &mut Interp, a: &[Value], signo: i32) -> EvalResult {
    let proc_arg = a.first().cloned().unwrap_or(Value::Nil);
    let target = get_process(i, &proc_arg)?;
    let pid = {
        let pb = target.borrow();
        if !matches!(pb.io, ProcIo::Child { .. }) {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not a subprocess")));
        }
        if pb.dead || matches!(pb.status, "exit" | "signal" | "closed" | "failed") {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not active")));
        }
        pb.pid
    };
    let lambda = match a.get(1) {
        Some(Value::Sym(id)) => i.symbol_name(*id) == "lambda",
        _ => false,
    };
    if pid > 0 && !lambda {
        if signo == libc::SIGCONT {
            // GNU psets the status to `run' and notifies the sentinel.
            let mut pb = target.borrow_mut();
            pb.pending_status = None;
            pb.status = "run";
        }
        unsafe { libc::kill(pid, signo) };
    }
    Ok(proc_arg)
}

fn f_interrupt_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU runs `interrupt-process-functions' until success; the
    // default member `internal-default-interrupt-process' signals SIGINT.
    run_hook_until_success(i, "interrupt-process-functions", a.as_slice().to_vec())
}

fn f_internal_default_interrupt_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: process_send_signal(SIGINT); returns the
    // PROCESS argument.
    process_send_signal(i, &a, libc::SIGINT)
}

fn f_kill_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: process_send_signal(SIGKILL) and returns PROCESS as
    // passed — a nil arg yields nil, not the resolved process.
    process_send_signal(i, &a, libc::SIGKILL)
}
fn f_quit_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    process_send_signal(i, &a, libc::SIGQUIT)
}
fn f_stop_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: a pipe/network/serial PROCESS OBJECT inhibits input
    // (`command' → t) instead of signalling; other arguments go through
    // process_send_signal(SIGTSTP), which also accepts names/buffers.
    if let Value::Process(p) = a.first().cloned().unwrap_or(Value::Nil) {
        let is_conn = {
            let pb = p.borrow();
            !matches!(pb.io, ProcIo::Child { .. })
        };
        if is_conn {
            let mut pb = p.borrow_mut();
            pb.pending_status = Some(pb.status);
            pb.status = "stop";
            if pb.kind == "network" {
                pb.command = Value::t();
            }
            return Ok(Value::Process(p.clone()));
        }
    }
    process_send_signal(i, &a, libc::SIGTSTP)
}
fn f_continue_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: resume input on a stopped pipe/network/serial
    // process object (`command' → nil); other arguments get SIGCONT
    // through process_send_signal.
    if let Value::Process(p) = a.first().cloned().unwrap_or(Value::Nil) {
        let is_conn = {
            let pb = p.borrow();
            !matches!(pb.io, ProcIo::Child { .. })
        };
        if is_conn {
            let mut pb = p.borrow_mut();
            if let Some(prev) = pb.pending_status.take() {
                pb.status = prev;
            }
            if pb.kind == "network" {
                pb.command = Value::Nil;
            }
            return Ok(Value::Process(p.clone()));
        }
    }
    process_send_signal(i, &a, libc::SIGCONT)
}

fn f_delete_process(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: PROCESS decodes via get_process — nil → current
    // buffer's process, names → process or buffer.
    let p = get_process(i, &arg(&a, 0))?;
    let ev;
    {
        let mut pb = p.borrow_mut();
        // A failed connect has its own notification, followed by a distinct
        // deleted notification when explicitly removed from the process list.
        if (pb.status == "failed" || pb.pending_status == Some("failed")) && !pb.dead {
            pb.reported = false;
            pb.status = "closed";
            pb.exit_status = 0;
            pb.connect_error = Value::Nil;
            pb.pending_status = None;
        }
        pb.dead = true;
        // GNU preserves an already-reported final status: deleting an
        // exited process leaves `process-status' showing `exit'.
        let final_seen =
            pb.reported || matches!(pb.status, "exit" | "signal" | "closed" | "failed");
        ev = match &mut pb.io {
            ProcIo::Child {
                child,
                input_fd,
                output_fd,
                tty_fd,
                stderr,
                pending_input,
                ..
            } => {
                *pending_input = VecDeque::new();
                let _ = child.kill();
                // Reap the child and release the pty — otherwise fds and
                // zombies accumulate until openpty starts failing.
                let _ = child.wait();
                input_fd.take();
                output_fd.take();
                tty_fd.take();
                stderr.take();
                if final_seen {
                    // Keep the observed status; the sentinel already ran.
                    ("signal", libc::SIGKILL)
                } else {
                    pb.status = "signal";
                    pb.exit_status = libc::SIGKILL;
                    ("signal", libc::SIGKILL)
                }
            }
            ProcIo::Pipe {
                read_fd,
                child_wfd,
                sink_fd,
            } => {
                unsafe {
                    libc::close(*read_fd);
                    if *child_wfd >= 0 {
                        libc::close(*child_wfd);
                    }
                    if *sink_fd >= 0 {
                        libc::close(*sink_fd);
                    }
                }
                // The process object can outlive deletion; never let its
                // Drop or a repeated delete close recycled descriptors.
                *read_fd = -1;
                *child_wfd = -1;
                *sink_fd = -1;
                pb.status = "closed";
                ("deleted", 0)
            }
            // GNU marks deleted network/serial processes "closed" and
            // closes the fd immediately — the peer sees EOF even while
            // Lisp references keep the process object alive.
            ProcIo::Net(s) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
                pb.io = ProcIo::None;
                pb.status = "closed";
                ("deleted", 0)
            }
            ProcIo::Connecting(_) => {
                pb.io = ProcIo::None;
                pb.status = "closed";
                ("deleted", 0)
            }
            ProcIo::LocalNet(s) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
                pb.status = "closed";
                ("deleted", 0)
            }
            ProcIo::Datagram(_) => {
                // Release the descriptor/path now, even if Lisp retains p.
                pb.io = ProcIo::None;
                pb.status = "closed";
                ("deleted", 0)
            }
            ProcIo::Serial(_) | ProcIo::Listen(_) | ProcIo::LocalListen(..) | ProcIo::None => {
                if !final_seen {
                    pb.status = "closed";
                }
                ("deleted", 0)
            }
        };
    }
    // GNU removes only the oldest alist cell for the process.
    if let Some(pos) = i.processes.iter().position(|q| Rc::ptr_eq(q, &p)) {
        i.processes.remove(pos);
    }
    // GNU delivers the final-state sentinel exactly once: a second
    // `delete-process' on an exited process is a silent no-op.
    if !p.borrow().reported {
        p.borrow_mut().reported = true;
        run_sentinel(i, &p, ev.0, ev.1)?;
    }
    Ok(Value::Nil)
}

fn f_process_running_child_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fprocess_running_child_p: PROCESS decodes through
    // `get_process' (buffer names, nil → current buffer's process), then
    // must be a live subprocess; the answer is the pty's foreground pgrp
    // — nil when it is the child itself, t when undeterminable.
    // PROCESS is optional: nil decodes to the current buffer's process.
    let p = get_process(i, a.first().unwrap_or(&Value::Nil))?;
    let (pid, master_fd) = {
        let pb = p.borrow();
        if !matches!(pb.io, ProcIo::Child { .. }) {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not a subprocess")));
        }
        if pb.dead || matches!(pb.status, "exit" | "signal" | "closed" | "failed") {
            let name = pb.name.clone();
            return Err(i.error(format!("Process {name} is not active")));
        }
        let fd = match &pb.io {
            ProcIo::Child { tty_fd, .. } => tty_fd.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            _ => -1,
        };
        (pb.pid, fd)
    };
    let gid = unsafe { libc::tcgetpgrp(master_fd) };
    if gid == pid {
        Ok(Value::Nil)
    } else if gid != -1 {
        Ok(Value::Int(gid as i128))
    } else {
        // TIOCGPGRP fails (no tty for a pipe child) — GNU returns t.
        Ok(Value::t())
    }
}

fn f_internal_default_process_filter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
        insert_coded_into_buffer(i, bid, pos, &a[1]);
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
                return Err(i.error(format!(
                    "{prog} exited with status {}",
                    o.status.code().unwrap_or(-1)
                )));
            }
            let text = decode(&o.stdout);
            let mut lines: Vec<&str> = text.split('\n').collect();
            if matches!(lines.last(), Some(l) if l.is_empty()) {
                lines.pop();
            }
            Ok(Value::list(lines.into_iter().map(Value::string).collect()))
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

fn f_process_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    process_lines(i, &a, false)
}
fn f_process_lines_ignore_status(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    process_lines(i, &a, true)
}

// ---------- system enumeration ----------

fn f_list_system_processes(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let mut pids = Vec::new();
    for pid in 1..=99999i32 {
        let r = unsafe { libc::kill(pid, 0) };
        if r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
            pids.push(Value::Int(pid as i128));
        }
    }
    Ok(Value::list(pids))
}

fn f_process_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pid = match &a[0] {
        Value::Process(p) => p.borrow().pid,
        Value::Int(n) => *n as i32,
        _ => return Err(i.wrong_type_mut("numberp", &a[0])),
    };
    // GNU's alist via ps(1): fixed key set in fixed order, time fields
    // as (hi lo usec psec) lists.
    // args goes last so all space-free fields split cleanly. macOS ps
    // has no egid/thcount/majflt keywords — gid/uid are effective ids.
    let fields = "rss=,vsz=,etime=,nice=,utime=,stime=,time=,ppid=,pgid=,state=,ucomm=,gid=,uid=,user=,args=";
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
            Some(Value::vector(vec![
                Value::Int(b[0] as i128),
                Value::Int(b[1] as i128),
                Value::Int(b[2] as i128),
                Value::Int(b[3] as i128),
                Value::Int(port),
            ]))
        }
        libc::AF_INET6 => {
            let sin6 = unsafe { &*(sa as *const libc::sockaddr_in6) };
            let b = sin6.sin6_addr.s6_addr;
            let mut elems: Vec<Value> = Vec::with_capacity(9);
            for pair in b.chunks(2) {
                elems.push(Value::Int(
                    (((pair[0] as u16) << 8) | pair[1] as u16) as i128,
                ));
            }
            elems.push(Value::Int(u16::from_be(sin6.sin6_port) as i128));
            Some(Value::vector(elems))
        }
        _ => None,
    }
}

fn f_network_interface_list(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_network_interface_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_format_network_address(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let omit_port = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    match &a[0] {
        Value::Vec(v) => {
            let v = v.borrow();
            let nums: Vec<i128> = v.iter().filter_map(|x| x.int()).collect();
            let dotted = |b: &[i128]| format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]);
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
            // GNU prints "<Family FAMILY>" for non-vector addresses; the
            // family must be an integer (formatted with %d in C).
            let items = a[0].list_to_vec().unwrap_or_default();
            match items.first().and_then(|v| v.int()) {
                Some(fam) => Ok(Value::string(format!("<Family {fam}>"))),
                None => Err(i.error_q("Format specifier doesn't match argument type", &[])),
            }
        }
        // AF_LOCAL socket paths are plain strings.
        Value::Str(_) => Ok(a[0].clone()),
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
    S!(
        "get-process",
        1,
        1,
        f_get_process,
        "Process named NAME or PROCESS itself."
    ),
    S!(
        "process-list",
        0,
        0,
        f_process_list,
        "List of live processes."
    ),
    S!(
        "list-processes",
        0,
        2,
        f_list_processes,
        "Show a process list buffer."
    ),
    S!("process-name", 1, 1, f_process_name, "Process name string."),
    S!(
        "process-buffer",
        1,
        1,
        f_process_buffer,
        "Buffer associated with PROCESS."
    ),
    S!(
        "set-process-buffer",
        2,
        2,
        f_set_process_buffer,
        "Set the process buffer."
    ),
    S!(
        "get-buffer-process",
        1,
        1,
        f_get_buffer_process,
        "Process whose buffer is BUFFER."
    ),
    S!(
        "process-command",
        1,
        1,
        f_process_command,
        "Command list of PROCESS."
    ),
    S!(
        "process-type",
        1,
        1,
        f_process_type,
        "Type symbol: real/network/serial/pipe."
    ),
    S!("process-id", 1, 1, f_process_id, "OS pid of PROCESS."),
    S!(
        "process-status",
        1,
        1,
        f_process_status,
        "Status symbol of PROCESS."
    ),
    S!(
        "process-exit-status",
        1,
        1,
        f_process_exit_status,
        "Exit code or signal of PROCESS."
    ),
    S!(
        "process-mark",
        1,
        1,
        f_process_mark,
        "Marker at process-buffer insertion point."
    ),
    S!(
        "process-contact",
        1,
        3,
        f_process_contact,
        "Contact info of PROCESS."
    ),
    S!(
        "process-plist",
        1,
        1,
        f_process_plist,
        "Property list of PROCESS."
    ),
    S!(
        "set-process-plist",
        2,
        2,
        f_set_process_plist,
        "Set process property list."
    ),
    S!(
        "process-get",
        2,
        2,
        f_process_get,
        "Get PROP from process plist."
    ),
    S!(
        "process-put",
        3,
        3,
        f_process_put,
        "Set PROP on process plist."
    ),
    S!(
        "process-query-on-exit-flag",
        1,
        1,
        f_process_query_on_exit_flag,
        "t if Emacs queries on exit."
    ),
    S!(
        "set-process-query-on-exit-flag",
        2,
        2,
        f_set_process_query_on_exit_flag,
        "Set query-on-exit flag."
    ),
    S!(
        "process-filter",
        1,
        1,
        f_process_filter,
        "Filter function of PROCESS."
    ),
    S!(
        "set-process-filter",
        2,
        2,
        f_set_process_filter,
        "Set process filter."
    ),
    S!(
        "process-sentinel",
        1,
        1,
        f_process_sentinel,
        "Sentinel of PROCESS."
    ),
    S!(
        "set-process-sentinel",
        2,
        2,
        f_set_process_sentinel,
        "Set process sentinel."
    ),
    S!(
        "process-tty-name",
        1,
        2,
        f_process_tty_name,
        "Controlling tty name or nil."
    ),
    S!(
        "process-coding-system",
        1,
        1,
        f_process_coding_system,
        "(DECODE . ENCODE) coding systems."
    ),
    S!(
        "set-process-coding-system",
        1,
        3,
        f_set_process_coding_system,
        "Set process coding systems."
    ),
    S!(
        "process-inherit-coding-system-flag",
        1,
        1,
        f_process_inherit_coding_system_flag,
        "Inherit-coding flag."
    ),
    S!(
        "set-process-inherit-coding-system-flag",
        2,
        2,
        f_set_process_inherit_coding_system_flag,
        "Set inherit-coding flag."
    ),
    S!(
        "set-process-window-size",
        3,
        3,
        f_set_process_window_size,
        "Tell PROCESS its window size."
    ),
    S!(
        "process-datagram-address",
        1,
        1,
        f_process_datagram_address,
        "Datagram address or nil."
    ),
    S!(
        "set-process-datagram-address",
        2,
        2,
        f_set_process_datagram_address,
        "Set datagram address."
    ),
    S!(
        "accept-process-output",
        0,
        4,
        f_accept_process_output,
        "Read pending process output."
    ),
    S!(
        "process-send-string",
        2,
        2,
        f_process_send_string,
        "Send STRING to PROCESS."
    ),
    S!(
        "process-send-region",
        3,
        3,
        f_process_send_region,
        "Send region text to PROCESS."
    ),
    S!(
        "send-string",
        2,
        2,
        f_process_send_string,
        "Send STRING to PROCESS."
    ),
    S!(
        "send-region",
        3,
        3,
        f_process_send_region,
        "Send region text to PROCESS."
    ),
    S!(
        "process-send-eof",
        0,
        1,
        f_process_send_eof,
        "Send EOF to PROCESS."
    ),
    S!(
        "delete-process",
        0,
        1,
        f_delete_process,
        "Delete PROCESS (kill if live)."
    ),
    S!(
        "signal-process",
        2,
        3,
        f_signal_process,
        "Send SIGNAL to process or pid."
    ),
    S!(
        "interrupt-process",
        0,
        2,
        f_interrupt_process,
        "Send SIGINT to PROCESS."
    ),
    S!(
        "internal-default-signal-process",
        2,
        3,
        f_internal_default_signal_process,
        "Default function to send PROCESS the signal with code SIGCODE."
    ),
    S!(
        "internal-default-interrupt-process",
        0,
        2,
        f_internal_default_interrupt_process,
        "Default function to interrupt PROCESS."
    ),
    S!(
        "kill-process",
        0,
        2,
        f_kill_process,
        "Send SIGKILL to PROCESS."
    ),
    S!(
        "quit-process",
        0,
        2,
        f_quit_process,
        "Send SIGQUIT to PROCESS."
    ),
    S!(
        "stop-process",
        0,
        2,
        f_stop_process,
        "Suspend PROCESS (SIGTSTP)."
    ),
    S!(
        "continue-process",
        0,
        2,
        f_continue_process,
        "Resume PROCESS (SIGCONT)."
    ),
    S!(
        "process-running-child-p",
        0,
        1,
        f_process_running_child_p,
        "Whether PROCESS has a running child."
    ),
    S!(
        "internal-default-process-filter",
        2,
        2,
        f_internal_default_process_filter,
        "Default filter: insert into buffer."
    ),
    S!(
        "internal-default-process-sentinel",
        2,
        2,
        f_internal_default_process_sentinel,
        "Default sentinel (no-op)."
    ),
    S!("process-lines", many 1, f_process_lines, "Run PROGRAM, return output lines."),
    S!("process-lines-ignore-status", many 1, f_process_lines_ignore_status, "process-lines ignoring exit status."),
    S!(
        "list-system-processes",
        0,
        0,
        f_list_system_processes,
        "Pids of all system processes."
    ),
    S!(
        "process-attributes",
        1,
        1,
        f_process_attributes,
        "Alist of attributes of PID."
    ),
    S!(
        "network-interface-list",
        0,
        2,
        f_network_interface_list,
        "Network interfaces and addresses."
    ),
    S!(
        "network-interface-info",
        1,
        1,
        f_network_interface_info,
        "(ADDR BCAST MASK HW) for IFNAME."
    ),
    S!(
        "format-network-address",
        1,
        2,
        f_format_network_address,
        "Format ADDRESS vector as string."
    ),
];

fn f_internal_default_process_sentinel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let p = want_proc(i, &a[0])?;
    let (bid, mark, name, status) = {
        let pb = p.borrow();
        (pb.buffer, pb.mark.clone(), pb.name.clone(), pb.status)
    };
    // GNU's internal run status covers live pipe/network "open" too.
    if matches!(status, "run" | "open") {
        return Ok(Value::Nil);
    }
    let Some(bid) = bid else {
        return Ok(Value::Nil);
    };
    let Some(buffer) = i.buffers.get(bid).filter(|b| b.borrow().live) else {
        return Ok(Value::Nil);
    };
    crate::buffer::primitives::chg_with_buffer(i, bid, |i| {
        let old_point = buffer.borrow().point();
        let at = mark
            .as_ref()
            .filter(|m| m.borrow().buffer.is_some())
            .map(|m| m.borrow().position)
            .unwrap_or_else(|| buffer.borrow().text_len());
        buffer.borrow_mut().set_point(at);
        let before = buffer.borrow().point();
        let read_only = i.intern("buffer-read-only");
        // GNU changes the buffer slot directly, without variable watchers.
        let saved_read_only = buffer.borrow_mut().locals.insert(read_only, Value::Nil);
        let insert = symv(i, "insert");
        let result = i.apply(
            &insert,
            vec![Value::string(format!("\nProcess {name} ")), a[1].clone()],
        );
        let after = buffer.borrow().point();
        match saved_read_only {
            Some(value) => {
                buffer.borrow_mut().locals.insert(read_only, value);
            }
            None => {
                buffer.borrow_mut().locals.shift_remove(&read_only);
            }
        }
        if buffer.borrow().live {
            if result.is_ok() {
                if let Some(mark) = &mark {
                    {
                        let mut m = mark.borrow_mut();
                        m.buffer = Some(bid);
                        m.position = after;
                    }
                    buffer.borrow_mut().register_marker(mark);
                }
            }
            buffer.borrow_mut().set_point(if old_point >= before {
                old_point + after.saturating_sub(before)
            } else {
                old_point
            });
        }
        result.map(|_| Value::Nil)
    })
}

/// GNU `make-network-process' advertises its capabilities through the
/// feature's `subfeatures' plist (process.c's `syms_of_process' calls
/// `provide' with the supported keyword matrix).  remacs implements
/// local (Unix-domain) and IP stream/datagram sockets with :server/:service;
/// register only those entries so `(featurep 'make-network-process
/// '(:family local))' reports t — `server.el' consults it to pick a
/// Unix socket over TCP.
pub(crate) fn install(i: &mut Interp) {
    // This is a GNU DEFVAR_INT: lexical let bindings must reach the native
    // constructor just like dynamic bindings do.
    let read_max = i.intern("read-process-output-max");
    i.obarray.symbol_mut(read_max).special = true;
    let mnp = i.intern("make-network-process");
    let subs = i.intern("subfeatures");
    let mkpair = |i: &mut Interp, a: &str, b: Value| Value::list(vec![Value::Sym(i.intern(a)), b]);
    let mut items: Vec<Value> = vec![
        mkpair(i, ":server", Value::t()),
        mkpair(i, ":service", Value::t()),
    ];
    for fam in ["ipv4", "local"] {
        let fs = Value::Sym(i.intern(fam));
        items.push(mkpair(i, ":family", fs));
    }
    let datagram = Value::Sym(i.intern("datagram"));
    items.push(mkpair(i, ":type", datagram));
    i.put_prop(mnp, subs, Value::list(items));
}

#[cfg(test)]
mod send_tests {
    use super::{send_whole_datagram, write_pending_input};
    use std::collections::VecDeque;
    use std::io::{Error, ErrorKind};

    #[test]
    fn datagram_sends_are_atomic_and_transient_errors_do_not_consume_bytes() {
        let packet = b"one-complete-packet";
        for kind in [ErrorKind::Interrupted, ErrorKind::WouldBlock] {
            assert_eq!(
                send_whole_datagram(packet, |bytes| {
                    assert_eq!(bytes, packet);
                    Err(kind.into())
                })
                .unwrap_err()
                .kind(),
                kind
            );
        }
        let mut sent = Vec::new();
        send_whole_datagram(packet, |bytes| {
            sent.push(bytes.to_vec());
            Ok(bytes.len())
        })
        .unwrap();
        assert_eq!(sent, vec![packet.to_vec()]);
        for n in [0, 1, packet.len() - 1] {
            let mut calls = 0;
            assert_eq!(
                send_whole_datagram(packet, |_| {
                    calls += 1;
                    Ok(n)
                })
                .unwrap_err()
                .kind(),
                ErrorKind::WriteZero
            );
            assert_eq!(
                calls, 1,
                "never send a short write's suffix as another packet"
            );
        }
        assert_eq!(
            send_whole_datagram(packet, |_| Err(Error::from_raw_os_error(libc::EMSGSIZE)))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EMSGSIZE)
        );
    }

    #[test]
    fn partial_writes_and_retries_preserve_exact_suffix() {
        let mut pending = VecDeque::from(b"abcdefgh".to_vec());
        let mut received = Vec::new();
        for result in [
            Ok(2),
            Err(ErrorKind::Interrupted),
            Err(ErrorKind::WouldBlock),
            Ok(3),
            Ok(3),
        ] {
            let before = pending.clone();
            let done = write_pending_input(&mut pending, |bytes| match result {
                Ok(n) => {
                    received.extend_from_slice(&bytes[..n]);
                    Ok(n)
                }
                Err(kind) => Err(kind.into()),
            });
            if let Err(kind) = result {
                assert_eq!(done.unwrap_err().kind(), kind);
                assert_eq!(pending, before);
            } else {
                assert_eq!(done.unwrap(), pending.is_empty());
            }
        }
        assert!(pending.is_empty());
        assert_eq!(pending.capacity(), 0);
        assert_eq!(received, b"abcdefgh");
    }

    #[test]
    fn nested_send_appends_after_unwritten_input() {
        let mut pending = VecDeque::with_capacity(8);
        pending.extend(b"abcdefgh");
        let mut received = Vec::new();
        assert!(
            !write_pending_input(&mut pending, |bytes| {
                received.extend_from_slice(&bytes[..6]);
                Ok(6)
            })
            .unwrap()
        );
        // Simulate another send during the wait; this also wraps the deque.
        pending.extend(b"1234");
        while !write_pending_input(&mut pending, |bytes| {
            received.extend_from_slice(bytes);
            Ok(bytes.len())
        })
        .unwrap()
        {}
        assert_eq!(received, b"abcdefgh1234");
        // The outer send must observe that the nested send already drained it.
        assert!(write_pending_input(&mut pending, |_| panic!("duplicate write")).unwrap());
    }

    #[test]
    fn zero_and_permanent_errors_do_not_report_success() {
        let original = VecDeque::from(b"remaining".to_vec());
        let mut pending = original.clone();
        assert_eq!(
            write_pending_input(&mut pending, |_| Ok(0))
                .unwrap_err()
                .kind(),
            ErrorKind::WriteZero
        );
        assert_eq!(pending, original);
        for errno in [libc::EPIPE, libc::EIO, libc::EBADF] {
            assert_eq!(
                write_pending_input(&mut pending, |_| Err(Error::from_raw_os_error(errno)))
                    .unwrap_err()
                    .raw_os_error(),
                Some(errno)
            );
            assert_eq!(pending, original);
        }
    }
}
