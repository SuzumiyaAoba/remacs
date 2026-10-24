//! Process invocation and accepted shutdown, independent of Lisp state.
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct Invocation {
    arguments: Vec<OsString>,
    executable: Option<PathBuf>,
    shutdown: Mutex<Option<Shutdown>>,
}

#[derive(Clone, Copy)]
struct Shutdown {
    code: i128,
    restart: bool,
}

static INVOCATION: OnceLock<Invocation> = OnceLock::new();

pub(crate) enum RestartError {
    NoArguments,
    UnknownExecutable,
    MissingExecutable(OsString),
}

/// Call at executable entry, before argument processing or Lisp bootstrap.
pub fn initialize() {
    INVOCATION.get_or_init(|| {
        let arguments: Vec<_> = std::env::args_os().collect();
        let executable = arguments.first().and_then(|name| find_executable(name));
        Invocation { arguments, executable, shutdown: Mutex::new(None) }
    });
}

/// GNU validates the original executable before running shutdown hooks.
pub(crate) fn validate_restart() -> Result<(), RestartError> {
    let Some(invocation) = INVOCATION.get().filter(|i| !i.arguments.is_empty()) else {
        return Err(RestartError::NoArguments);
    };
    let Some(executable) = &invocation.executable else {
        return Err(RestartError::UnknownExecutable);
    };
    if !file_access(executable, libc::F_OK) {
        return Err(RestartError::MissingExecutable(invocation.arguments[0].clone()));
    }
    Ok(())
}

/// Only an accepted exit reaches here. Inner exits from hooks win first.
pub(crate) fn accept_shutdown(code: i128, restart: bool) {
    if let Some(invocation) = INVOCATION.get() {
        let mut shutdown = invocation.shutdown.lock().unwrap_or_else(|e| e.into_inner());
        shutdown.get_or_insert(Shutdown { code, restart });
    }
}

/// Finish after a front end has restored its terminal/window resources.
/// Batch exits can reach this directly, without running Lisp cleanup.
pub fn exit(code: i128) -> ! {
    if let Some(invocation) = INVOCATION.get() {
        let shutdown = *invocation.shutdown.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(shutdown) = shutdown {
            if shutdown.restart {
                if let Some(executable) = &invocation.executable {
                    #[cfg(unix)]
                    let error = reexecute(executable, &invocation.arguments[1..]);
                    #[cfg(not(unix))]
                    let error = match std::process::Command::new(executable)
                        .args(&invocation.arguments[1..]).status() {
                        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
                        Err(error) => error,
                    };
                    let message = error.raw_os_error().map(|number| unsafe {
                        std::ffi::CStr::from_ptr(libc::strerror(number)).to_string_lossy().into_owned()
                    }).unwrap_or_else(|| error.to_string());
                    eprintln!("{}: Unable to re-execute Emacs: {message}", invocation.arguments[0].to_string_lossy());
                }
            }
            std::process::exit(shutdown.code as i32);
        }
    }
    std::process::exit(code as i32);
}

#[cfg(unix)]
fn reexecute(executable: &Path, arguments: &[OsString]) -> std::io::Error {
    use std::os::unix::ffi::OsStrExt;
    // execvp preserves the process environment and ignored signal dispositions.
    // Command::exec also performs child setup (including resetting SIGPIPE).
    let program = std::ffi::CString::new(executable.as_os_str().as_bytes()).unwrap();
    let arguments: Vec<_> = std::iter::once(program.clone()).chain(arguments.iter()
        .map(|arg| std::ffi::CString::new(arg.as_bytes()).unwrap())).collect();
    let mut pointers: Vec<_> = arguments.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());
    unsafe { libc::execvp(program.as_ptr(), pointers.as_ptr()); }
    std::io::Error::last_os_error()
}

fn follow_if_symlink(path: PathBuf) -> PathBuf {
    if std::fs::read_link(&path).is_ok() {
        std::fs::canonicalize(&path).unwrap_or(path)
    } else {
        path
    }
}

#[cfg(unix)]
fn file_access(path: &Path, mode: libc::c_int) -> bool {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .is_ok_and(|path| unsafe { libc::access(path.as_ptr(), mode) == 0 })
}

#[cfg(not(unix))]
fn file_access(path: &Path, _mode: libc::c_int) -> bool { path.exists() }

#[cfg(unix)]
fn default_path() -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    let length = unsafe { libc::confstr(libc::_CS_PATH, std::ptr::null_mut(), 0) };
    if length == 0 { return None; }
    let mut bytes = vec![0; length];
    unsafe { libc::confstr(libc::_CS_PATH, bytes.as_mut_ptr().cast(), length); }
    bytes.pop();
    Some(OsString::from_vec(bytes))
}

#[cfg(unix)]
fn find_executable(name: &OsStr) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    if name.as_bytes().contains(&b'/') {
        return Some(follow_if_symlink(PathBuf::from(name)));
    }
    let path = std::env::var_os("PATH").filter(|p| !p.is_empty()).or_else(default_path)?;
    for directory in std::env::split_paths(&path) {
        let directory = if directory.as_os_str().is_empty() { Path::new(".") } else { &directory };
        let candidate = directory.join(name);
        if file_access(&candidate, libc::X_OK) && candidate.is_file() {
            return Some(follow_if_symlink(candidate));
        }
    }
    // GNU returns a non-null empty candidate after an unsuccessful search.
    Some(PathBuf::new())
}

#[cfg(not(unix))]
fn find_executable(_name: &OsStr) -> Option<PathBuf> { std::env::current_exe().ok() }
