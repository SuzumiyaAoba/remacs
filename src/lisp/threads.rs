//! Cooperative Lisp threads, running on the interpreter's OS thread.
//!
//! Each coroutine owns separate `Interp` storage. At a switch the runtime
//! moves by value to the next stack; a vacant, unevaluated interpreter stays
//! behind. Suspended Rust `&mut Interp` references therefore never alias the
//! interpreter which another stack is executing.

use super::*;
use crate::lisp::value::{MutexRef, ThreadRef};
use corosensei::{Coroutine, CoroutineResult, Yielder};
use std::any::Any;
use std::cell::Cell;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

pub(crate) type ThreadYielder = Yielder<Box<Interp>, Packet>;
type Fiber = Coroutine<Box<Interp>, Packet, Packet>;

pub(crate) struct Packet {
    runtime: Box<Interp>,
    wait: Wait,
    panic: Option<Box<dyn Any + Send>>,
    exit: Option<i128>,
}

#[derive(Clone)]
pub(crate) enum Wait {
    Runnable,
    Sleep(Instant),
    Join(ThreadRef),
    Mutex(MutexRef),
    Condition(Rc<Cell<bool>>),
}

impl Wait {
    fn ready(&self) -> bool {
        match self {
            Self::Runnable => true,
            Self::Sleep(deadline) => Instant::now() >= *deadline,
            Self::Join(thread) => thread.borrow().finished,
            Self::Mutex(mutex) => mutex.borrow().owner.is_none(),
            Self::Condition(notified) => notified.get(),
        }
    }
}

struct Task {
    thread: ThreadRef,
    fiber: Fiber,
    wait: Wait,
}

#[derive(Default)]
pub(crate) struct Scheduler {
    tasks: Vec<Task>,
    main_events: Vec<Value>,
}

pub(crate) fn signal_main(i: &mut Interp, symbol: Value, data: Value) {
    let event = i.q("thread-event");
    let sender = Value::Thread(i.threads[i.current_thread].clone());
    i.thread_scheduler
        .main_events
        .push(Value::list(vec![Value::Sym(event), sender, symbol, data]));
}

pub(crate) fn drain_main_events(i: &mut Interp) -> Result<(), Flow> {
    if i.current_thread == 0 {
        let events = std::mem::take(&mut i.thread_scheduler.main_events);
        for event in events {
            let handler = i.q("thread-handle-event");
            i.apply(&Value::Sym(handler), vec![event])?;
        }
    }
    Ok(())
}

// These fields describe the active Rust/Lisp call stack, rather than the
// shared Lisp runtime. Keep their suspended values on that coroutine's stack.
macro_rules! thread_context {
    ($($field:ident: $ty:ty = $empty:expr),* $(,)?) => {
        struct ThreadContext {
            native: crate::lisp::builtins::comp::ThreadState,
            $($field: $ty,)*
        }
        impl ThreadContext {
            fn take(i: &mut Interp) -> Self {
                i.synchronize_quit_request();
                exchange_bindings(i, true);
                Self {
                    native: crate::lisp::builtins::comp::take_thread_state(),
                    $($field: std::mem::replace(&mut i.$field, $empty),)*
                }
            }
            fn install(self, i: &mut Interp) {
                $(i.$field = self.$field;)*
                crate::lisp::builtins::comp::restore_thread_state(self.native);
                exchange_bindings(i, false);
            }
        }
    }
}

thread_context! {
    specbind: Vec<SpecBind> = Vec::new(),
    lexenv: LexEnv = None,
    current_buffer: usize = 0,
    current_thread: usize = 0,
    thread_yielder: Option<NonNull<ThreadYielder>> = None,
    eval_depth: usize = 0,
    explicit_eval_depth: usize = 0,
    max_lisp_eval_depth: usize = 1600,
    quit_flag: bool = false,
    bc_quit_counter: Option<Rc<std::cell::Cell<u8>>> = None,
    unwind_quit: Option<Value> = None,
    catch_tags: Vec<Value> = Vec::new(),
    command_args: Vec<Value> = Vec::new(),
    interactive_call_depth: u32 = 0,
    defining_symbol: Option<SymId> = None,
    recursion_depth: usize = 0,
    last_error: Option<(Value, Value)> = None,
    signaling_function: Value = Value::Nil,
    match_data: Option<MatchData> = None,
    running_asynch_code: bool = false,
    saved_async_match_data: Option<Option<MatchData>> = None,
    pending_insert_behind: Vec<Value> = Vec::new(),
    pending_insert_front: Vec<Value> = Vec::new(),
    pending_overlay_hooks: Vec<(Value, Value)> = Vec::new(),
    combine_after_change_list: Vec<(usize, usize, i128)> = Vec::new(),
    combine_after_change_buffer: Option<usize> = None,
    loads_in_progress: Vec<String> = Vec::new(),
    load_stream: Option<Rc<RefCell<crate::lisp::load::LoadStream>>> = None,
    load_stream_symbol: Option<SymId> = None,
    loading_dumped: bool = false,
    dumped_call_depth: usize = 0,
    reading_elc: bool = false,
    boot_loading: bool = false,
    bc_depth: usize = 0,
    macroexp_call_depth: usize = 0,
    current_lambda_dumped: bool = false,
    current_body_compiled: bool = false,
    compiled_macro_expansion: bool = false,
    expand_unadvised_depth: usize = 0,
    loading_compiled_library: bool = false,
    handler_bindings: Vec<(Value, Value, usize)> = Vec::new(),
    case_handlers: Vec<crate::lisp::error::CaseHandler> = Vec::new(),
    lisp_stack: Vec<(Value, Argv)> = Vec::new(),
    pending_frames: Vec<crate::lisp::backtrace::PendingFrame> = Vec::new(),
    cur_head_form: Value = Value::Nil,
    last_error_stack: RefCell<Vec<crate::lisp::backtrace::Frame>> = RefCell::new(Vec::new()),
}

/// Swap the live binding cells with their saved values, without invoking
/// variable watchers. Reverse order exposes global defaults; forward order
/// reinstates this thread's bindings, including nested bindings of one name.
fn exchange_bindings(i: &mut Interp, outgoing: bool) {
    let mut bindings = std::mem::take(&mut i.specbind);
    let n = bindings.len();
    for step in 0..n {
        let sb = &mut bindings[if outgoing { n - 1 - step } else { step }];
        if let Some(buffer) = sb.buf {
            if let Some(b) = i.buffers.get(buffer) {
                let mut b = b.borrow_mut();
                match (&mut sb.old, b.locals.get_mut(&sb.sym)) {
                    (Some(old), Some(live)) => std::mem::swap(old, live),
                    (None, Some(_)) => sb.old = b.locals.shift_remove(&sb.sym),
                    (Some(_), None) => {
                        b.locals.insert(sb.sym, sb.old.take().unwrap());
                    }
                    (None, None) => {}
                }
                if sb.sym == i.bfn_sym {
                    b.file_name = match b.locals.get(&sb.sym) {
                        Some(Value::Str(s)) => Some(s.borrow().clone()),
                        _ => None,
                    };
                }
            }
        } else if let Some(old) = &mut sb.old {
            std::mem::swap(old, &mut i.obarray.symbol_mut(sb.sym).value);
            let restored = i.obarray.symbol(sb.sym).value.clone();
            i.sync_undo_inhibit(sb.sym, &restored);
        }
    }
    i.specbind = bindings;
}

pub(crate) fn spawn(i: &mut Interp, function: Value, thread: ThreadRef) -> Result<(), Flow> {
    let index = i.threads.len() - 1;
    let buffer = i.current_buffer;
    let object = thread.clone();
    // Rust evaluator frames are larger than GNU's C frames. Reserve the
    // same virtual stack allowance used by the interpreter QA runner.
    let stack = corosensei::stack::DefaultStack::new(64 * 1024 * 1024)
        .map_err(|_| i.error("Could not start a new thread"))?;
    let fiber = Coroutine::with_stack(stack, move |yielder, mut runtime: Box<Interp>| {
        runtime.current_thread = index;
        runtime.current_buffer = buffer;
        runtime.thread_yielder = Some(NonNull::from(yielder));
        // Restore the runtime even if a Rust primitive panics. The resumer
        // propagates that panic only after reinstalling its own context.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check_signal(&mut runtime)?;
            runtime.apply(&function, vec![])
        }));
        let mut panic = None;
        let mut exit = None;
        {
            let mut t = object.borrow_mut();
            t.finished = true;
            t.alive = false;
            t.blocker = None;
            match outcome {
                Ok(Ok(value)) => t.result = Some(value),
                Ok(Err(Flow::Signal(signal))) => {
                    let error = signal.condition;
                    t.last_error = Some(error.clone());
                    runtime.thread_last_error = error;
                }
                Ok(Err(Flow::Quit)) => {
                    let error = Value::list(vec![Value::Sym(sym::QUIT)]);
                    t.last_error = Some(error.clone());
                    runtime.thread_last_error = error;
                }
                Ok(Err(Flow::Throw(tag, value))) => {
                    let no_catch = runtime.q("no-catch");
                    let error = Value::list(vec![Value::Sym(no_catch), tag, value]);
                    t.last_error = Some(error.clone());
                    runtime.thread_last_error = error;
                }
                Ok(Err(Flow::Exit(status))) => exit = Some(status),
                Err(payload) => panic = Some(payload),
            }
        }
        // Forced coroutine destruction unwinds a suspended stack while
        // its runtime (and TLS) belongs to the resumer. The local runtime
        // is vacant in that case; do not take the resumer's native state.
        if !runtime.threads.is_empty() {
            drop(ThreadContext::take(&mut runtime));
        }
        Packet {
            runtime,
            wait: Wait::Runnable,
            panic,
            exit,
        }
    });
    i.thread_scheduler.tasks.push(Task {
        thread,
        fiber,
        wait: Wait::Runnable,
    });
    Ok(())
}

fn signaled(thread: &ThreadRef) -> bool {
    thread
        .borrow()
        .pending_signal
        .as_ref()
        .is_some_and(|(s, _)| !s.is_nil())
}

pub(crate) fn check_signal(i: &mut Interp) -> Result<(), Flow> {
    let pending = i.threads[i.current_thread]
        .borrow_mut()
        .pending_signal
        .take();
    if let Some((symbol, data)) = pending {
        if !symbol.is_nil() {
            crate::lisp::builtins::evalfn::f_signal(i, vec![symbol, data].into())?;
        }
    }
    Ok(())
}

/// Run one ready worker until it yields or exits. Scheduler ownership stays
/// on the resumer's stack; workers may queue additional tasks in the runtime.
fn run_one(i: &mut Interp) -> Result<bool, Flow> {
    let mut scheduler = std::mem::take(&mut i.thread_scheduler);
    let Some(index) = scheduler
        .tasks
        .iter()
        .position(|t| t.wait.ready() || signaled(&t.thread))
    else {
        i.thread_scheduler = scheduler;
        return Ok(false);
    };
    let mut task = scheduler.tasks.remove(index);
    let parent = ThreadContext::take(i);
    let runtime = Box::new(std::mem::replace(i, Interp::vacant()));
    let (packet, done) = match task.fiber.resume(runtime) {
        CoroutineResult::Yield(packet) => (packet, false),
        CoroutineResult::Return(packet) => (packet, true),
    };
    *i = *packet.runtime;
    scheduler.tasks.append(&mut i.thread_scheduler.tasks);
    scheduler
        .main_events
        .append(&mut i.thread_scheduler.main_events);
    if !done {
        task.wait = packet.wait;
        scheduler.tasks.push(task);
    }
    i.thread_scheduler = scheduler;
    parent.install(i);
    if let Some(panic) = packet.panic {
        std::panic::resume_unwind(panic);
    }
    if let Some(status) = packet.exit {
        return Err(Flow::Exit(status));
    }
    Ok(true)
}

fn suspend(i: &mut Interp, wait: Wait) -> Result<(), Flow> {
    let yielder = i
        .thread_yielder
        .expect("worker has a live coroutine yielder");
    let context = ThreadContext::take(i);
    let packet = Packet {
        runtime: Box::new(std::mem::replace(i, Interp::vacant())),
        wait,
        panic: None,
        exit: None,
    };
    // SAFETY: installed by the coroutine entry closure, pointing to its
    // stable yielder on this same stack. It is removed from the runtime
    // before suspension, restored only on this coroutine, and never sent
    // to another OS thread. The coroutine owns the stack until unwound.
    let runtime = unsafe { yielder.as_ref().suspend(packet) };
    *i = *runtime;
    context.install(i);
    check_signal(i)
}

pub(crate) fn yield_now(i: &mut Interp) -> Result<(), Flow> {
    if i.thread_yielder.is_some() {
        suspend(i, Wait::Runnable)
    } else {
        run_one(i)?;
        check_signal(i)
    }
}

pub(crate) fn wait(i: &mut Interp, wait: Wait, blocker: Option<Value>) -> Result<(), Flow> {
    let thread = i.threads[i.current_thread].clone();
    thread.borrow_mut().blocker = blocker;
    let result = (|| {
        if i.thread_yielder.is_some() {
            suspend(i, wait)
        } else {
            while !wait.ready() && !signaled(&thread) {
                let ran = run_one(i)?;
                crate::lisp::builtins::evalfn::thread_wait_events(i)?;
                if !ran {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            check_signal(i)
        }
    })();
    thread.borrow_mut().blocker = None;
    result
}

/// Sleep in a worker releases execution to its resumer. Main-thread waits
/// retain their existing timer/event pump, scheduling workers between polls.
pub(crate) fn sleep_worker(i: &mut Interp, deadline: Instant) -> Result<bool, Flow> {
    if i.thread_yielder.is_some() {
        wait(i, Wait::Sleep(deadline), None)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// A short event-loop polling interval also gives other Lisp threads a turn.
pub(crate) fn pause(i: &mut Interp, duration: Duration) -> Result<(), Flow> {
    if !sleep_worker(i, Instant::now() + duration)? {
        yield_now(i)?;
        std::thread::sleep(duration);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn large_stack(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn dropping_a_suspended_worker_releases_its_captures() {
        large_stack(|| {
            let mut i = Interp::new();
            let value = i.eval_str("(setq qa196-payload (vector 196))").unwrap();
            let Value::Vec(vector) = value else {
                panic!("vector")
            };
            let weak = Rc::downgrade(&vector);
            drop(vector);
            let thread = i
                .eval_str(
                    "(eval '(let ((capture qa196-payload)) \
                (make-thread (lambda () (thread-yield) capture))) t)",
                )
                .unwrap();
            i.eval_str("(setq qa196-payload nil)").unwrap();
            yield_now(&mut i).unwrap();
            assert!(weak.upgrade().is_some());
            drop(i);
            assert!(weak.upgrade().is_none());
            // A thread object may outlive its interpreter, without retaining
            // the discarded execution stack or its lexical environment.
            drop(thread);
        });
    }

    fn panic_after_yield(i: &mut Interp, _: Argv) -> EvalResult {
        yield_now(i)?;
        panic!("qa196 native primitive panic");
    }

    static PANIC: crate::lisp::value::Subr = crate::lisp::value::Subr {
        name: "qa196-panic",
        arity: Arity::Range { min: 0, max: 0 },
        func: panic_after_yield,
        doc: "",
    };

    #[test]
    fn native_panic_restores_the_resumers_runtime_and_bindings() {
        large_stack(|| {
            let mut i = Interp::new();
            let panic_symbol = i.q("qa196-panic");
            i.fset(panic_symbol, Value::Subr(&PANIC));
            i.eval_str(
                "(defvar qa196-special 'global) \
                (make-thread (lambda () (let ((qa196-special 'child)) (qa196-panic))))",
            )
            .unwrap();
            let special = i.q("qa196-special");
            let parent = i.q("parent");
            let buffer = i.current_buffer;
            i.specbind(special, Value::Sym(parent)).unwrap();
            yield_now(&mut i).unwrap();
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| yield_now(&mut i)));
            assert!(result.is_err());
            assert_eq!(i.current_thread, 0);
            assert_eq!(i.current_buffer, buffer);
            assert_eq!(i.specbind_depth(), 1);
            assert!(crate::lisp::eq_values(
                &i.symbol_value(special),
                &Value::Sym(parent)
            ));
            i.unbind(1).unwrap();
            let global = i.q("global");
            assert!(crate::lisp::eq_values(
                &i.symbol_value(special),
                &Value::Sym(global)
            ));
        });
    }
}
