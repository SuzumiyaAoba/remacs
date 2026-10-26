//! Native Darwin process metadata, with the SDK's kinfo_proc layout isolated in C.
use super::{EvalResult, Interp, Value, symv};

#[repr(C)]
struct ProcessInfo {
    flags: u64,
    user_ns: u64,
    system_ns: u64,
    pageins: u64,
    virtual_size: u64,
    resident_size: u64,
    start_sec: i64,
    start_usec: i64,
    euid: u32,
    egid: u32,
    ppid: i32,
    pgrp: i32,
    tpgid: i32,
    nice: i32,
    thcount: i32,
    state: [u8; 8],
    comm: [u8; 4096],
    user: [u8; 256],
    group: [u8; 256],
    tty: [u8; 1024],
}

unsafe extern "C" {
    fn remacs_macos_system_processes(pids: *mut i32, capacity: usize) -> usize;
    fn remacs_macos_process_attributes(
        pid: i32,
        out: *mut ProcessInfo,
        out_size: usize,
        args: *mut u8,
        args_size: usize,
    ) -> i32;
}

fn string(bytes: &[u8]) -> Value {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    Value::string(super::decode(&bytes[..end]))
}

fn time(nanoseconds: i128) -> Value {
    let seconds = nanoseconds.div_euclid(1_000_000_000);
    let fraction = nanoseconds.rem_euclid(1_000_000_000);
    Value::list(vec![
        Value::Int(seconds.div_euclid(65536)),
        Value::Int(seconds.rem_euclid(65536)),
        Value::Int(fraction / 1000),
        Value::Int((fraction % 1000) * 1000),
    ])
}

pub(super) fn process_attributes(i: &mut Interp, pid: i32) -> EvalResult {
    // The C ABI struct contains only integers and byte arrays; all-zero is valid.
    let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
    let mut args = vec![0u8; 262144];
    if unsafe {
        remacs_macos_process_attributes(
            pid,
            &mut info,
            std::mem::size_of::<ProcessInfo>(),
            args.as_mut_ptr(),
            args.len(),
        )
    } == 0
    {
        return Ok(Value::Nil);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as i128)
        .unwrap_or(0);
    let start = i128::from(info.start_sec) * 1_000_000_000 + i128::from(info.start_usec) * 1000;
    let mut attributes = Vec::new();
    let mut add = |key: &str, value: Value| attributes.push(Value::cons(symv(i, key), value));
    if info.flags & 32 != 0 {
        add("args", string(&args));
    }
    if info.flags & 16 != 0 {
        add("thcount", Value::Int(i128::from(info.thcount)));
        add("rss", Value::integer(i128::from(info.resident_size / 1024)));
        add(
            "vsize",
            Value::integer(i128::from(info.virtual_size / 1024)),
        );
    }
    add("etime", time(now - start));
    add("start", time(start));
    add("nice", Value::Int(i128::from(info.nice)));
    if info.flags & 8 != 0 {
        add("majflt", Value::integer(i128::from(info.pageins)));
        add(
            "time",
            time(i128::from(info.user_ns) + i128::from(info.system_ns)),
        );
        add("stime", time(i128::from(info.system_ns)));
        add("utime", time(i128::from(info.user_ns)));
    }
    add("tpgid", Value::Int(i128::from(info.tpgid)));
    if info.flags & 64 != 0 {
        add("ttname", string(&info.tty));
    }
    add("pgrp", Value::Int(i128::from(info.pgrp)));
    add("ppid", Value::Int(i128::from(info.ppid)));
    add("state", string(&info.state));
    add("comm", string(&info.comm));
    if info.flags & 4 != 0 {
        add("group", string(&info.group));
    }
    add("egid", Value::Int(i128::from(info.egid)));
    if info.flags & 2 != 0 {
        add("user", string(&info.user));
    }
    add("euid", Value::Int(i128::from(info.euid)));
    Ok(Value::list(attributes))
}

pub(super) fn list_system_processes() -> EvalResult {
    let capacity = unsafe { remacs_macos_system_processes(std::ptr::null_mut(), 0) };
    if capacity == 0 {
        return Ok(Value::Nil);
    }
    let mut pids = vec![0i32; capacity];
    let count = unsafe { remacs_macos_system_processes(pids.as_mut_ptr(), pids.len()) };
    pids.truncate(count);
    Ok(Value::list(
        pids.into_iter()
            .map(|pid| Value::Int(i128::from(pid)))
            .collect(),
    ))
}
