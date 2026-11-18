#!/usr/bin/env python3
"""Run one command with finite time, memory, CPU, and output limits."""

import argparse
import ctypes
import ctypes.util
import errno
import os
from pathlib import Path
import resource
import selectors
import signal
import subprocess
import sys
import tempfile
import time


PRESETS = {
    # These are group-wide RSS limits and wall-clock limits. CPU is enforced
    # per process by RLIMIT_CPU; the wall-clock limit also bounds total CPU.
    "build": {"timeout": 1800, "rss_mib": 8192, "cpu_secs": 1800, "log_mib": 32},
    "check": {"timeout": 1800, "rss_mib": 8192, "cpu_secs": 1800, "log_mib": 32},
    "focused-test": {"timeout": 120, "rss_mib": 4096, "cpu_secs": 120, "log_mib": 20},
    "full-test": {"timeout": 3600, "rss_mib": 8192, "cpu_secs": 3600, "log_mib": 32},
}
POLL_SECONDS = 0.2
TERMINATE_GRACE_SECONDS = 3
LAST_PGID = None
LAST_CLEANED = True


class GuardInterrupted(Exception):
    """Raised for interrupt signals so the child process group is cleaned up."""


def _signal_handler(signum, _frame):
    raise GuardInterrupted(signum)


if sys.platform == "darwin":
    _libproc_path = ctypes.util.find_library("proc")
    _libproc = ctypes.CDLL(_libproc_path, use_errno=True) if _libproc_path else None
else:
    _libproc = None


if _libproc is not None:
    class _ProcBsdInfo(ctypes.Structure):
        _fields_ = [
            ("pbi_flags", ctypes.c_uint32),
            ("pbi_status", ctypes.c_uint32),
            ("pbi_xstatus", ctypes.c_uint32),
            ("pbi_pid", ctypes.c_uint32),
            ("pbi_ppid", ctypes.c_uint32),
            ("pbi_uid", ctypes.c_uint32),
            ("pbi_gid", ctypes.c_uint32),
            ("pbi_ruid", ctypes.c_uint32),
            ("pbi_rgid", ctypes.c_uint32),
            ("pbi_svuid", ctypes.c_uint32),
            ("pbi_svgid", ctypes.c_uint32),
            ("rfu_1", ctypes.c_uint32),
            ("pbi_comm", ctypes.c_char * 16),
            ("pbi_name", ctypes.c_char * 32),
            ("pbi_nfiles", ctypes.c_uint32),
            ("pbi_pgid", ctypes.c_uint32),
            ("pbi_pjobc", ctypes.c_uint32),
            ("e_tdev", ctypes.c_uint32),
            ("e_tpgid", ctypes.c_uint32),
            ("pbi_nice", ctypes.c_int32),
            ("pbi_start_tvsec", ctypes.c_uint64),
            ("pbi_start_tvusec", ctypes.c_uint64),
        ]

    class _ProcTaskInfo(ctypes.Structure):
        _fields_ = [
            ("pti_virtual_size", ctypes.c_uint64),
            ("pti_resident_size", ctypes.c_uint64),
            ("pti_total_user", ctypes.c_uint64),
            ("pti_total_system", ctypes.c_uint64),
            ("pti_threads_user", ctypes.c_uint64),
            ("pti_threads_system", ctypes.c_uint64),
            ("pti_policy", ctypes.c_int32),
            ("pti_faults", ctypes.c_int32),
            ("pti_pageins", ctypes.c_int32),
            ("pti_cow_faults", ctypes.c_int32),
            ("pti_messages_sent", ctypes.c_int32),
            ("pti_messages_received", ctypes.c_int32),
            ("pti_syscalls_mach", ctypes.c_int32),
            ("pti_syscalls_unix", ctypes.c_int32),
            ("pti_csw", ctypes.c_int32),
            ("pti_threadnum", ctypes.c_int32),
            ("pti_numrunning", ctypes.c_int32),
            ("pti_priority", ctypes.c_int32),
        ]

    _libproc.proc_listpids.argtypes = [
        ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_int,
    ]
    _libproc.proc_listpids.restype = ctypes.c_int
    _libproc.proc_pidinfo.argtypes = [
        ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int,
    ]
    _libproc.proc_pidinfo.restype = ctypes.c_int


def _mac_group_members(pgid):
    capacity = 4096 * ctypes.sizeof(ctypes.c_int)
    while capacity <= 16 * 1024 * 1024:
        pids = ctypes.create_string_buffer(capacity)
        ctypes.set_errno(0)
        size = _libproc.proc_listpids(2, pgid, pids, capacity)  # PROC_PGRP_ONLY
        if size < 0:
            return None
        if size < capacity:
            break
        capacity *= 2
    else:
        return None

    result = []
    count = size // ctypes.sizeof(ctypes.c_int)
    for pid in (ctypes.c_int * count).from_buffer(pids):
        if pid <= 0:
            continue
        bsd = _ProcBsdInfo()
        ctypes.set_errno(0)
        got = _libproc.proc_pidinfo(pid, 3, 0, ctypes.byref(bsd), ctypes.sizeof(bsd))
        if got != ctypes.sizeof(bsd):
            if ctypes.get_errno() == errno.ESRCH:
                continue
            return None
        if bsd.pbi_status == 5:  # SZOMB has no resident task to account for.
            continue
        task = _ProcTaskInfo()
        ctypes.set_errno(0)
        got = _libproc.proc_pidinfo(pid, 4, 0, ctypes.byref(task), ctypes.sizeof(task))
        if got != ctypes.sizeof(task):
            if ctypes.get_errno() == errno.ESRCH:
                continue
            return None
        rss_kib = (task.pti_resident_size + 1023) // 1024
        result.append((pid, rss_kib))
    return result


def _linux_group_members(pgid):
    result = []
    try:
        for entry in Path("/proc").iterdir():
            if not entry.name.isdigit():
                continue
            try:
                fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
                # Fields after comm start at field 3: state, ppid, pgrp.
                if int(fields[2]) != pgid or fields[0] == "Z":
                    continue
                pages = int((entry / "statm").read_text().split()[1])
                rss_kib = pages * os.sysconf("SC_PAGE_SIZE") // 1024
                result.append((int(entry.name), rss_kib))
            except (FileNotFoundError, ProcessLookupError, PermissionError,
                    ValueError, IndexError):
                continue
    except OSError:
        return None
    return result


def group_members(pgid):
    if _libproc is not None:
        return _mac_group_members(pgid)
    if sys.platform.startswith("linux"):
        return _linux_group_members(pgid)
    try:
        output = subprocess.run(
            ["ps", "-axo", "pid=,pgid=,rss=,stat="],
            capture_output=True, text=True, check=True, timeout=3,
        ).stdout
    except Exception:
        return None
    result = []
    for row in output.splitlines():
        fields = row.split()
        if len(fields) != 4 or fields[3].startswith("Z"):
            continue
        try:
            if int(fields[1]) == pgid:
                result.append((int(fields[0]), int(fields[2])))
        except ValueError:
            continue
    return result


def _wait_for_group_exit(pgid, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        members = group_members(pgid)
        if members == []:
            return True
        time.sleep(0.1)
    return group_members(pgid) == []


def terminate_group(proc):
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    clean = _wait_for_group_exit(proc.pid, TERMINATE_GRACE_SECONDS)
    if not clean:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    try:
        proc.wait(timeout=TERMINATE_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    return _wait_for_group_exit(proc.pid, TERMINATE_GRACE_SECONDS)


def supervise(argv, timeout, rss_mib, cpu_secs, log_mib, log_path=None):
    global LAST_PGID, LAST_CLEANED
    LAST_CLEANED = False
    keep_log = log_path is not None
    if log_path is None:
        log_fd, log_path = tempfile.mkstemp(prefix="remacs-guard-", suffix=".log")
        os.close(log_fd)
    else:
        log_path = str(Path(log_path).resolve())
        Path(log_path).parent.mkdir(parents=True, exist_ok=True)
    log_limit = log_mib * 1024 * 1024

    def apply_limits():
        _soft_limit, hard_limit = resource.getrlimit(resource.RLIMIT_CPU)
        requested_hard = cpu_secs + 5
        if hard_limit != resource.RLIM_INFINITY:
            requested_hard = min(requested_hard, hard_limit)
        resource.setrlimit(resource.RLIMIT_CPU,
                           (min(cpu_secs, requested_hard), requested_hard))

    started = time.monotonic()
    reason = "exited"
    cleaned = True
    proc = None
    try:
        with open(log_path, "wb", buffering=0) as output:
            proc = subprocess.Popen(
                argv,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,
                preexec_fn=apply_limits,
            )
            LAST_PGID = proc.pid
            selector = selectors.DefaultSelector()
            assert proc.stdout is not None
            os.set_blocking(proc.stdout.fileno(), False)
            selector.register(proc.stdout, selectors.EVENT_READ)
            log_size = 0
            stream_open = True
            next_progress = started + 30
            try:
                while True:
                    for key, _events in selector.select(POLL_SECONDS):
                        try:
                            data = os.read(key.fd, 64 * 1024)
                        except BlockingIOError:
                            continue
                        if not data:
                            selector.unregister(key.fileobj)
                            stream_open = False
                            continue
                        available = log_limit - log_size
                        if len(data) > available:
                            if available > 0:
                                output.write(data[:available])
                                log_size += available
                            reason = f"log-size limit ({log_mib} MiB)"
                            break
                        output.write(data)
                        log_size += len(data)
                    if reason != "exited":
                        break

                    status = proc.poll()
                    members = group_members(proc.pid)
                    now = time.monotonic()
                    elapsed = now - started
                    if elapsed >= timeout:
                        reason = f"wall-time limit ({timeout:g}s)"
                        break
                    if members is None:
                        reason = "process-group RSS monitor unavailable"
                        break
                    rss_kib = sum(rss for _pid, rss in members)
                    if rss_kib > rss_mib * 1024:
                        reason = (f"RSS limit ({rss_kib / 1024:.0f} MiB > "
                                  f"{rss_mib} MiB)")
                        break
                    if status is not None and members:
                        reason = "child processes remained after command exit"
                        break
                    if status is not None and not stream_open:
                        break
                    if now >= next_progress:
                        print(f"[guard] running elapsed={elapsed:.0f}s "
                              f"RSS={rss_kib / 1024:.0f}/{rss_mib} MiB "
                              f"log={log_size / 1024 / 1024:.1f}/{log_mib} MiB",
                              flush=True)
                        next_progress = now + 30
            finally:
                selector.close()
                proc.stdout.close()

            if reason != "exited":
                cleaned = terminate_group(proc)
            else:
                proc.wait()
                if proc.returncode == -getattr(signal, "SIGXCPU", -999):
                    reason = f"CPU-time limit ({cpu_secs}s per process)"
            proc.wait()
    except GuardInterrupted:
        reason = "supervisor interrupted"
        raise
    finally:
        if proc is not None and proc.stdout is not None and not proc.stdout.closed:
            proc.stdout.close()
        if proc is not None:
            members = group_members(proc.pid)
            if proc.poll() is None or members != []:
                cleaned = terminate_group(proc) and cleaned
        LAST_CLEANED = cleaned
        if (reason != "exited" or not cleaned or
                (proc is not None and proc.returncode != 0) or keep_log):
            print(f"[guard] log kept at {log_path}", file=sys.stderr, flush=True)
        else:
            try:
                os.unlink(log_path)
            except OSError:
                pass

    return {
        "returncode": proc.returncode if proc is not None else 127,
        "reason": reason,
        "cleaned": cleaned,
        "seconds": round(time.monotonic() - started, 2),
        "log": log_path,
    }


def _print_log_tail(path, max_bytes=64 * 1024):
    try:
        with open(path, "rb") as log:
            size = os.fstat(log.fileno()).st_size
            log.seek(max(0, size - max_bytes))
            tail = log.read(max_bytes).decode(errors="replace")
    except OSError:
        return
    if tail:
        print("[guard] final log output:", file=sys.stderr)
        print(tail, file=sys.stderr, end="" if tail.endswith("\n") else "\n")


def _sleeping_process_command():
    code = "import os,time; os.fork(); time.sleep(30)"
    return [sys.executable, "-c", code]


def self_test():
    if not hasattr(os, "fork"):
        raise SystemExit("self-test requires a POSIX platform with fork()")
    result = supervise(_sleeping_process_command(), 1, 512, 10, 2)
    assert result["reason"].startswith("wall-time limit") and result["cleaned"], result
    assert group_members(LAST_PGID) == [], LAST_PGID
    print("timeout cleanup: passed")

    previous_alarm_handler = signal.signal(signal.SIGALRM, _signal_handler)
    try:
        signal.setitimer(signal.ITIMER_REAL, 1)
        try:
            supervise(_sleeping_process_command(), 10, 512, 20, 2)
            raise AssertionError("supervisor interruption was not received")
        except GuardInterrupted:
            pass
        assert group_members(LAST_PGID) == [], LAST_PGID
        print("supervisor interruption cleanup: passed")
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_alarm_handler)


def _env_number(name, default, cast):
    value = os.environ.get(name)
    if value is None:
        return default
    try:
        return cast(value)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{name} must be numeric") from exc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preset", choices=sorted(PRESETS), default="focused-test")
    parser.add_argument("--timeout", type=float)
    parser.add_argument("--rss-mib", type=int)
    parser.add_argument("--cpu-secs", type=int)
    parser.add_argument("--log-mib", type=int)
    parser.add_argument("--log", help="write command output to this file and keep it on success")
    parser.add_argument("--self-test", action="store_true",
                        help="check timeout and interrupt cleanup with sleeping dummy processes")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, _signal_handler)
    if args.self_test:
        self_test()
        return 0

    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("command required after --")

    defaults = PRESETS[args.preset]
    timeout = args.timeout
    rss_mib = args.rss_mib
    cpu_secs = args.cpu_secs
    log_mib = args.log_mib
    try:
        if timeout is None:
            timeout = _env_number("REMACS_GUARD_TIMEOUT", defaults["timeout"], float)
        if rss_mib is None:
            rss_mib = _env_number("REMACS_GUARD_RSS_MIB", defaults["rss_mib"], int)
        if cpu_secs is None:
            cpu_secs = _env_number("REMACS_GUARD_CPU_SECS", defaults["cpu_secs"], int)
        if log_mib is None:
            log_mib = _env_number("REMACS_GUARD_LOG_MIB", defaults["log_mib"], int)
    except argparse.ArgumentTypeError as exc:
        parser.error(str(exc))
    if min(timeout, rss_mib, cpu_secs, log_mib) <= 0:
        parser.error("all resource limits must be greater than zero")

    print(f"[guard] preset={args.preset} wall<={timeout:g}s RSS<={rss_mib} MiB "
          f"CPU<={cpu_secs}s/process output<={log_mib} MiB", flush=True)
    try:
        result = supervise(command, timeout, rss_mib, cpu_secs, log_mib, args.log)
    except GuardInterrupted as exc:
        clean_text = "ok" if LAST_CLEANED else "FAILED"
        print(f"[guard] interrupted by signal {exc.args[0]}; cleanup={clean_text}",
              file=sys.stderr, flush=True)
        if not LAST_CLEANED:
            return 125
        return 128 + int(exc.args[0])
    print(f"[guard] result={result['reason']} rc={result['returncode']} "
          f"elapsed={result['seconds']}s cleanup={'ok' if result['cleaned'] else 'FAILED'}",
          flush=True)
    if result["reason"] != "exited" or not result["cleaned"]:
        _print_log_tail(result["log"])
        return 125
    if result["returncode"] < 0:
        _print_log_tail(result["log"])
        return 128 - result["returncode"]
    if result["returncode"] != 0:
        _print_log_tail(result["log"])
    return result["returncode"]


if __name__ == "__main__":
    raise SystemExit(main())
