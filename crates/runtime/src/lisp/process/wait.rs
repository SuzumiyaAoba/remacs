//! Non-panicking timeout accounting and the host select timeout contract.
use super::*;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub(super) struct Timeout {
    seconds: Option<f64>,
    started: Instant,
    probe: Option<(UnixStream, UnixStream)>,
    observed_exits: Vec<ProcessRef>,
}

impl Timeout {
    pub(super) fn new(seconds: Option<f64>) -> Self {
        Self {
            seconds: seconds.map(|s| s.max(0.0)),
            started: Instant::now(),
            probe: None,
            observed_exits: Vec::new(),
        }
    }

    fn remaining(&self) -> Option<f64> {
        self.seconds
            .map(|s| (s - self.started.elapsed().as_secs_f64()).max(0.0))
    }

    pub(super) fn expired(&self) -> bool {
        self.remaining().is_some_and(|remaining| remaining <= 0.0)
    }

    pub(super) fn validate(&mut self, i: &mut Interp, inhibit_timers: bool) -> Result<(), Flow> {
        let remaining = self.remaining();
        // Ordinary waits need no extra syscall. An hour is safely within
        // the host select range, as is any smaller timer-bounded wait.
        if remaining.is_some_and(|s| s <= 3600.0) {
            return Ok(());
        }
        let timer = if inhibit_timers {
            None
        } else {
            super::super::builtins::evalfn::next_timer_delay(i)
        };
        let effective = match (remaining, timer) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let Some(seconds) = effective.filter(|s| *s > 3600.0) else {
            return Ok(());
        };
        // GNU's SIGCHLD notification zeroes this select timeout before
        // dispatching data/status. Reaping here only caches std::Child's
        // result; the normal poll path still owns status and sentinels.
        self.observed_exits.retain(|process| {
            let process = process.borrow();
            !process.dead && !process.reported
        });
        let mut new_exit = false;
        for process in &i.processes {
            if self
                .observed_exits
                .iter()
                .any(|seen| Rc::ptr_eq(seen, process))
            {
                continue;
            }
            let mut value = process.borrow_mut();
            if value.dead || value.reported {
                continue;
            }
            if let ProcIo::Child { child, .. } = &mut value.io {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    new_exit = true;
                    // Retain identity so a removed process's allocation
                    // cannot be reused as a different notification.
                    self.observed_exits.push(process.clone());
                }
            }
        }
        if new_exit {
            return Ok(());
        }
        self.validate_select(seconds)
            .map_err(|error| crate::editor::report_file_errno(i, "Failed select", vec![], &error))
    }

    fn validate_select(&mut self, seconds: f64) -> std::io::Result<()> {
        if self.probe.is_none() {
            let (read, mut write) = UnixStream::pair()?;
            write.write_all(&[0])?;
            // Keep the readable byte queued. The writer stays owned so
            // this descriptor cannot turn into EOF between validations.
            self.probe = Some((read, write));
        }
        let fd = self.probe.as_ref().unwrap().0.as_raw_fd();
        if fd < 0 || fd as usize >= libc::FD_SETSIZE {
            // Never let FD_SET index beyond its fixed-size allocation.
            return Err(std::io::Error::from_raw_os_error(libc::EMFILE));
        }
        let timeout = match Duration::try_from_secs_f64(seconds) {
            Ok(duration) if duration.as_secs() < libc::time_t::MAX as u64 => libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
            // GNU dtotimespec saturates positive overflow/infinity.
            _ => libc::timespec {
                tv_sec: libc::time_t::MAX,
                tv_nsec: 999_999_999,
            },
        };
        loop {
            let mut ready = unsafe { std::mem::zeroed::<libc::fd_set>() };
            unsafe {
                libc::FD_SET(fd, &mut ready);
            }
            // A queued byte makes this a validation, never a long sleep.
            // This preserves each platform's select limits instead of
            // confusing Rust Duration/Instant limits with GNU's errors.
            let result = unsafe {
                libc::pselect(
                    fd + 1,
                    &mut ready,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &timeout,
                    std::ptr::null(),
                )
            };
            if result >= 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
