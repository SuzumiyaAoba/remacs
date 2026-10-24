//! CPU-time and allocation profiles consumed by the unmodified profiler.el.
//!
//! Sampling happens at interpreter safe points rather than in an async
//! signal handler: Rc and the evaluator cannot be accessed from SIGPROF.
//! Call boundaries flush allocation deltas before changing the stack, so a
//! bulk primitive's allocations and CPU time belong to that primitive.

use rustc_hash::FxHashMap;

use super::builtins::hashfn::{hash_key_for, table_key};
use super::error::EvalResult;
use super::value::{Argv, FIXNUM_MAX, HashKey, HashRef, HashTest, LispHash, Value};
use super::{Interp, memory};

#[derive(Default)]
pub(crate) struct State {
    cpu: Option<Log>,
    memory: Option<Log>,
    interval: u128,
    cpu_clock: u128,
    memory_bytes: u64,
    polls: usize,
}

struct Sample {
    trace: Vec<Value>,
    count: i128,
}

struct Log {
    size: usize,
    depth: usize,
    samples: FxHashMap<Vec<HashKey>, Sample>,
    discarded: i128,
}

impl Log {
    fn record(&mut self, frames: &[(HashKey, Value)], count: i128) {
        if count == 0 {
            return;
        }
        if self.size == 0 {
            self.discarded = self.discarded.saturating_add(count).min(FIXNUM_MAX);
            return;
        }
        let mut key: Vec<_> = frames
            .iter()
            .take(self.depth)
            .map(|(key, _)| key.clone())
            .collect();
        key.resize(self.depth, HashKey::Nil);
        if let Some(sample) = self.samples.get_mut(&key) {
            sample.count = sample.count.saturating_add(count).min(FIXNUM_MAX);
            return;
        }
        if self.samples.len() >= self.size {
            // Retain the most frequent half, and expose lost weight in
            // the same special entry that GNU's UI understands.
            let mut counts: Vec<_> = self.samples.values().map(|sample| sample.count).collect();
            let middle = (counts.len() - 1) / 2;
            counts.select_nth_unstable(middle);
            let median = counts[middle];
            self.samples.retain(|_, sample| {
                if sample.count > median {
                    return true;
                }
                self.discarded = self.discarded.saturating_add(sample.count).min(FIXNUM_MAX);
                false
            });
        }
        let mut trace: Vec<_> = frames
            .iter()
            .take(self.depth)
            .map(|(_, value)| value.clone())
            .collect();
        trace.resize(self.depth, Value::Nil);
        self.samples.insert(
            key,
            Sample {
                trace,
                count: count.min(FIXNUM_MAX),
            },
        );
    }
}

fn config(i: &Interp) -> Log {
    let setting = |name: &str, default: usize| {
        i.intern_soft(name)
            .and_then(|id| i.symbol_value(id).int())
            .map_or(default, |n| n.clamp(0, i32::MAX as i128) as usize)
    };
    Log {
        size: setting("profiler-log-size", 10_000),
        depth: setting("profiler-max-stack-depth", 16),
        samples: FxHashMap::default(),
        discarded: 0,
    }
}

fn cpu_time() -> Option<u128> {
    #[cfg(unix)]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        for clock in [
            libc::CLOCK_THREAD_CPUTIME_ID,
            libc::CLOCK_PROCESS_CPUTIME_ID,
        ] {
            if unsafe { libc::clock_gettime(clock, &mut time) } == 0 {
                return Some(time.tv_sec as u128 * 1_000_000_000 + time.tv_nsec as u128);
            }
        }
    }
    None
}

pub(crate) fn start_cpu(i: &mut Interp, args: Argv) -> EvalResult {
    if i.cpu_profiler {
        return Err(i.error("CPU profiler is already running"));
    }
    // GNU creates a pending log even when interval validation fails.
    if i.profiling.cpu.is_none() {
        i.profiling.cpu = Some(config(i));
    }
    let interval = match args[0] {
        Value::Int(n) if n > 0 && n <= FIXNUM_MAX => n as u128,
        _ => return Err(i.error("Invalid sampling interval")),
    };
    let Some(now) = cpu_time() else {
        return Err(i.error("Unable to start profiler timer"));
    };
    i.profiling.interval = interval;
    i.profiling.cpu_clock = now;
    i.cpu_profiler = true;
    Ok(Value::t())
}

pub(crate) fn start_memory(i: &mut Interp, _args: Argv) -> EvalResult {
    if i.memory_profiler {
        return Err(i.error("Memory profiler is already running"));
    }
    if i.profiling.memory.is_none() {
        i.profiling.memory = Some(config(i));
    }
    i.profiling.memory_bytes = memory::allocated_bytes();
    i.memory_profiler = true;
    Ok(Value::t())
}

pub(crate) fn stop_cpu(i: &mut Interp, _args: Argv) -> EvalResult {
    i.profiler_checkpoint();
    let running = std::mem::replace(&mut i.cpu_profiler, false);
    Ok(Value::from_bool(running))
}

pub(crate) fn stop_memory(i: &mut Interp, _args: Argv) -> EvalResult {
    i.profiler_checkpoint();
    let running = std::mem::replace(&mut i.memory_profiler, false);
    Ok(Value::from_bool(running))
}

fn export(i: &mut Interp, cpu: bool) -> EvalResult {
    i.profiler_checkpoint();
    let running = if cpu {
        i.cpu_profiler
    } else {
        i.memory_profiler
    };
    let log = if cpu {
        i.profiling.cpu.take()
    } else {
        i.profiling.memory.take()
    };
    let Some(log) = log else {
        return Ok(Value::Nil);
    };
    let table = memory::allocate(LispHash::new(HashTest::Equal), Value::Hash);
    for sample in log.samples.into_values() {
        export_sample(i, &table, sample.trace, sample.count)?;
    }
    if log.discarded > 0 {
        let discarded = Value::Sym(i.q("Discarded Samples"));
        export_sample(i, &table, vec![discarded, Value::Nil], log.discarded)?;
    }
    if running {
        if cpu {
            i.profiling.cpu = Some(config(i));
            if let Some(now) = cpu_time() {
                i.profiling.cpu_clock = now;
            }
        } else {
            i.profiling.memory = Some(config(i));
            i.profiling.memory_bytes = memory::allocated_bytes();
        }
    }
    Ok(Value::Hash(table))
}

fn export_sample(
    i: &mut Interp,
    table: &HashRef,
    trace: Vec<Value>,
    count: i128,
) -> Result<(), super::Flow> {
    let trace = Value::vector(trace);
    let key = table_key(i, table, &trace)?;
    let mut table = table.borrow_mut();
    // Equal export keys can coalesce independently sampled closures.
    // Preserve all their weight rather than overwriting an earlier row.
    let old = table.map.get(&key).and_then(Value::int).unwrap_or(0);
    table.map.insert(
        key.clone(),
        Value::Int(old.saturating_add(count).min(FIXNUM_MAX)),
    );
    table.put_key(key, trace);
    Ok(())
}

pub(crate) fn cpu_log(i: &mut Interp, _args: Argv) -> EvalResult {
    export(i, true)
}
pub(crate) fn memory_log(i: &mut Interp, _args: Argv) -> EvalResult {
    export(i, false)
}

impl Interp {
    /// Flush against the old call stack before every push or pop.
    #[inline]
    pub(crate) fn profiler_checkpoint(&mut self) {
        if self.cpu_profiler || self.memory_profiler {
            self.profiler_record();
        }
    }

    /// Tight interpreted/byte-code loops also provide CPU sample points.
    #[inline]
    pub(crate) fn profiler_poll(&mut self) {
        if self.cpu_profiler || self.memory_profiler {
            self.profiling.polls = self.profiling.polls.wrapping_add(1);
            if self.profiling.polls & 63 == 0 {
                self.profiler_record();
            }
        }
    }

    fn profiler_record(&mut self) {
        let cpu_count = if self.cpu_profiler {
            cpu_time().map_or(0, |now| {
                let count = now.saturating_sub(self.profiling.cpu_clock) / self.profiling.interval;
                self.profiling.cpu_clock += count * self.profiling.interval;
                count.min(FIXNUM_MAX as u128) as i128
            })
        } else {
            0
        };
        let memory_count = if self.memory_profiler {
            let bytes = memory::allocated_bytes();
            let count = bytes.saturating_sub(self.profiling.memory_bytes);
            self.profiling.memory_bytes = bytes;
            count.min(FIXNUM_MAX as u64) as i128
        } else {
            0
        };
        if cpu_count == 0 && memory_count == 0 {
            return;
        }
        let depth = self
            .profiling
            .cpu
            .as_ref()
            .map_or(0, |log| log.depth)
            .max(self.profiling.memory.as_ref().map_or(0, |log| log.depth));
        let frames: Vec<_> = self
            .lisp_stack
            .iter()
            .rev()
            .take(depth)
            .map(|(function, _)| (hash_key_for(self, function, HashTest::Eq), function.clone()))
            .collect();
        if let Some(log) = &mut self.profiling.cpu {
            log.record(&frames, cpu_count);
        }
        if let Some(log) = &mut self.profiling.memory {
            log.record(&frames, memory_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_logs_preserve_evicted_weight_and_pad_traces() {
        let mut log = Log {
            size: 2,
            depth: 3,
            samples: FxHashMap::default(),
            discarded: 0,
        };
        for (function, count) in [(1, 10), (2, 20), (3, 30), (3, 5)] {
            log.record(&[(HashKey::Sym(function), Value::Sym(function))], count);
        }
        assert!(log.samples.len() <= 2);
        assert!(log.discarded > 0);
        assert_eq!(
            log.samples
                .values()
                .map(|sample| sample.count)
                .sum::<i128>()
                + log.discarded,
            65
        );
        for sample in log.samples.values() {
            assert_eq!(sample.trace.len(), 3);
            assert!(sample.trace[1..].iter().all(Value::is_nil));
        }
    }
}
