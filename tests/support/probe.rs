//! Run instrumented probes in bounded, independent copies of their test binary.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// Keep the existing serialization of probes that compete for ptys/ports.
// Only the parent holds this lock; a stuck child cannot retain it forever.
static PROBE_LOCK: Mutex<()> = Mutex::new(());
const CHILD: &str = "REMACS_TEST_PROBE_CHILD";
const TIMEOUT: Duration = Duration::from_secs(120);

fn with_slot<T>(slot: &Mutex<()>, body: impl FnOnce() -> T) -> T {
    let _guard = slot.lock().unwrap_or_else(|error| error.into_inner());
    body()
}

struct Log(PathBuf);
impl Log {
    fn new() -> io::Result<(Self, File)> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "remacs-probe-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let log = Self(root);
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(log.0.join("output"))?;
        Ok((log, file))
    }
}
impl Drop for Log {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) struct Report {
    pub(crate) status: ExitStatus,
    pub(crate) timed_out: bool,
    elapsed: Duration,
    pub(crate) tail: String,
}

pub(crate) fn run_command(command: &mut Command, timeout: Duration) -> io::Result<Report> {
    let (log, mut output) = Log::new()?;
    command
        .current_dir(&log.0)
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output.try_clone()?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let start = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if start.elapsed() >= timeout {
            // The unreaped child still owns this PID/group. Kill its workers
            // too, before wait() releases the PID for reuse.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(not(unix))]
            child.kill()?;
            break (child.wait()?, true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let end = output.metadata()?.len();
    output.seek(SeekFrom::Start(end.saturating_sub(16 * 1024)))?;
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    Ok(Report {
        status,
        timed_out,
        elapsed: start.elapsed(),
        tail: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

// redirect-debugging-output can change the child's stderr. Restore it before
// reporting a Lisp failure or printing the completion handshake.
#[cfg(unix)]
struct SavedStderr(i32);
#[cfg(unix)]
impl SavedStderr {
    fn new() -> io::Result<Self> {
        let fd = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(fd))
        }
    }
}
#[cfg(unix)]
impl Drop for SavedStderr {
    fn drop(&mut self) {
        unsafe {
            libc::dup2(self.0, libc::STDERR_FILENO);
            libc::close(self.0);
        }
    }
}

pub fn run(name: &'static str, file: &'static str, body: impl FnOnce() + Send + 'static) {
    let complete = format!("[probe-complete] {name} {file}");
    if std::env::var(CHILD).ok().as_deref() == Some(name) {
        #[cfg(unix)]
        let stderr = SavedStderr::new().expect("save probe stderr");
        let result = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(body)
            .expect("spawn probe thread")
            .join();
        #[cfg(unix)]
        drop(stderr);
        if let Err(payload) = result {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("probe thread panicked");
            eprintln!("{file}: {message}");
            std::panic::resume_unwind(payload);
        }
        eprintln!("{complete}");
        return;
    }
    let report = with_slot(&PROBE_LOCK, || {
        run_command(
            Command::new(std::env::current_exe().expect("probe test binary"))
                .args(["--exact", name, "--nocapture", "--test-threads=1"])
                .env(CHILD, name),
            TIMEOUT,
        )
        .expect("run isolated probe")
    });
    assert!(
        !report.timed_out && report.status.success() && report.tail.contains(&complete),
        "probe {file} ({name}) failed after {:?}; timed_out={}; status={}\n{}",
        report.elapsed,
        report.timed_out,
        report.status,
        report.tail
    );
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn deadline_terminates_workers_and_releases_the_probe_slot() {
        let slot = Mutex::new(());
        let report = with_slot(&slot, || {
            run_command(
                Command::new("/bin/sh").args(["-c", "sleep 30 & printf '%s' \"$!\"; wait"]),
                Duration::from_millis(150),
            )
            .unwrap()
        });
        assert!(report.timed_out);
        assert!(report.elapsed < Duration::from_secs(5));
        let pid = report.tail.trim().parse::<i32>().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "worker survived deadline");
        let _slot = slot.try_lock().expect("deadline retained probe lock");
        let next = run_command(
            Command::new("/bin/sh").args(["-c", "printf next"]),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(next.status.success());
        assert_eq!(next.tail, "next");
    }

    #[test]
    fn child_failures_keep_their_status_and_diagnostics() {
        let report = run_command(
            Command::new("/bin/sh").args(["-c", "printf failure >&2; exit 7"]),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!report.timed_out);
        assert_eq!(report.status.code(), Some(7));
        assert_eq!(report.tail, "failure");
    }
}
