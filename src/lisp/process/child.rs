//! Owned descriptors for real subprocess transports.
use super::ProcIo;
use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::{Command, Stdio};

fn cloexec(fd: &OwnedFd) -> io::Result<()> {
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe returned two new descriptors; both acquire ownership
    // before any fallible work, including close-on-exec setup.
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    cloexec(&read)?;
    cloexec(&write)?;
    Ok((read, write))
}

fn pty() -> io::Result<(OwnedFd, OwnedFd, String)> {
    let mut master = -1;
    let mut slave = -1;
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty succeeded; each endpoint has exactly one owner.
    let master_owned = unsafe { OwnedFd::from_raw_fd(master) };
    let slave_owned = unsafe { OwnedFd::from_raw_fd(slave) };
    cloexec(&master_owned)?;
    cloexec(&slave_owned)?;
    let name = unsafe {
        let name = libc::ttyname(slave);
        let name = if name.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(name)
                .to_string_lossy()
                .into_owned()
        };
        // GNU's child_setup_tty configures the subprocess side of the
        // pty: canonical input stays ON with VEOF=^D, so process-send-eof's
        // ^D makes the child see end-of-file (and flushes a partial
        // line); echo is off, output postprocessing on but without
        // NL→CR-NL mapping, delays and tab expansion off, 8-bit clean,
        // and erase/kill processing disabled.
        let mut tio: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(slave, &mut tio) == 0 {
            tio.c_oflag |= libc::OPOST;
            tio.c_oflag &= !libc::ONLCR;
            tio.c_oflag &= !(libc::NLDLY
                | libc::CRDLY
                | libc::TABDLY
                | libc::BSDLY
                | libc::VTDLY
                | libc::FFDLY);
            tio.c_lflag &= !libc::ECHO;
            tio.c_lflag |= libc::ISIG;
            tio.c_iflag &= !libc::ISTRIP;
            tio.c_oflag &= !libc::TAB3;
            tio.c_cflag = (tio.c_cflag & !libc::CSIZE) | libc::CS8;
            tio.c_lflag |= libc::ICANON;
            tio.c_cc[libc::VEOF] = 0o04; // Control-D
            tio.c_cc[libc::VERASE] = libc::_POSIX_VDISABLE;
            tio.c_cc[libc::VKILL] = libc::_POSIX_VDISABLE;
            libc::tcsetattr(slave, libc::TCSANOW, &tio);
        }
        name
    };
    Ok((master_owned, slave_owned, name))
}

pub(super) fn spawn(
    arguments: &[String],
    requested_input: bool,
    requested_output: bool,
    stderr_pipe: Option<OwnedFd>,
    split_stderr: bool,
) -> io::Result<(ProcIo, Option<String>)> {
    // Like GNU, fall back to pipes if the requested pty is unavailable.
    let terminal = if requested_input || requested_output {
        pty().ok()
    } else {
        None
    };
    let pty_input = requested_input && terminal.is_some();
    let pty_output = requested_output && terminal.is_some();
    let tty_name = terminal.as_ref().map(|(_, _, name)| name.clone());
    let tty_fd = terminal
        .as_ref()
        .map(|(master, _, _)| master.try_clone())
        .transpose()?;
    let input = if pty_input {
        Some(terminal.as_ref().unwrap().0.try_clone()?)
    } else {
        None
    };
    let (output, child_output) = if pty_output {
        let (master, slave, _) = terminal.as_ref().unwrap();
        (master.try_clone()?, slave.try_clone()?)
    } else {
        pipe()?
    };
    let child_error = if let Some(fd) = stderr_pipe {
        cloexec(&fd)?;
        Stdio::from(fd)
    } else if split_stderr {
        Stdio::piped()
    } else {
        Stdio::from(child_output.try_clone()?)
    };
    let child_input = if pty_input {
        Stdio::from(terminal.as_ref().unwrap().1.try_clone()?)
    } else {
        Stdio::piped()
    };
    let mut command = Command::new(&arguments[0]);
    command
        .args(&arguments[1..])
        .stdin(child_input)
        .stdout(Stdio::from(child_output))
        .stderr(child_error);
    let mut child = command.spawn()?;
    // Command owns the child-side pipe descriptors and must not keep a
    // writer alive after spawn, or EOF could never reach the reader.
    drop(command);
    drop(terminal);
    let input = input.unwrap_or_else(|| child.stdin.take().expect("piped stdin").into());
    let stderr = child.stderr.take();
    super::nonblock_fd(input.as_raw_fd());
    super::nonblock_fd(output.as_raw_fd());
    if let Some(stderr) = &stderr {
        super::nonblock_fd(stderr.as_raw_fd());
    }
    Ok((
        ProcIo::Child {
            child,
            input_fd: Some(input),
            output_fd: Some(output),
            tty_fd,
            pty_input,
            pty_output,
            stderr,
            pending_input: VecDeque::new(),
        },
        tty_name,
    ))
}
