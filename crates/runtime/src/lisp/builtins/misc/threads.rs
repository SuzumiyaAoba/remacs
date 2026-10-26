//! Lisp thread, mutex and condition-variable primitives over the shared scheduler.

use crate::lisp::builtins::arg;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    value::Value,
};

// ---------- threads ----------

pub(super) fn want_thread(
    i: &mut Interp,
    v: &Value,
) -> Result<crate::lisp::value::ThreadRef, Flow> {
    match v {
        Value::Thread(t) => Ok(t.clone()),
        other => Err(i.wrong_type_mut("threadp", other)),
    }
}

pub(super) fn f_threadp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Thread(_))))
}

pub(super) fn f_all_threads(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU lists only threads that haven't finished (a finished thread
    // drops out even before `thread-join' reaps it).
    let ts: Vec<Value> = i
        .threads
        .iter()
        .filter(|t| !t.borrow().finished)
        .map(|t| Value::Thread(t.clone()))
        .collect();
    Ok(Value::list(ts))
}

pub(super) fn f_current_thread(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Thread(i.threads[i.current_thread].clone()))
}

pub(super) fn f_thread_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(t.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

pub(super) fn f_thread_live_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(Value::from_bool(t.borrow().alive))
}

pub(super) fn f_make_thread(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fun = a[0].clone();
    let name = match a.get(1) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let t = crate::lisp::memory::allocate(
        crate::lisp::value::Thread {
            name,
            alive: true,
            result: None,
            last_error: None,
            pending_signal: None,
            blocker: None,
            finished: false,
        },
        crate::lisp::Value::Thread,
    );
    i.threads.push(t.clone());
    if let Err(error) = crate::lisp::eval::threads::spawn(i, fun, t.clone()) {
        i.threads.pop();
        return Err(error);
    }
    Ok(Value::Thread(t))
}

pub(super) fn f_thread_join(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    if std::rc::Rc::ptr_eq(&t, &i.threads[i.current_thread]) {
        return Err(i.error("Cannot join current thread"));
    }
    // GNU captures a target's pending signal before waiting, even if the
    // target subsequently handles it. A completed target retains it.
    let pending = t.borrow().pending_signal.clone();
    if !t.borrow().finished {
        crate::lisp::eval::threads::wait(
            i,
            crate::lisp::eval::threads::Wait::Join(t.clone()),
            Some(Value::Thread(t.clone())),
        )?;
    }
    let result = t.borrow().result.clone();
    if let Some((symbol, data)) = pending {
        if !symbol.is_nil() {
            return crate::lisp::builtins::evalfn::f_signal(i, vec![symbol, data].into());
        }
    }
    Ok(result.unwrap_or(Value::Nil))
}

pub(super) fn f_thread_yield(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::eval::threads::yield_now(i)?;
    Ok(Value::Nil)
}

pub(super) fn f_thread_last_error(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let error = i.thread_last_error.clone();
    if arg(&a, 0).truthy() {
        i.thread_last_error = Value::Nil;
    }
    Ok(error)
}

// ---------- mutexes and condition variables (cooperative model) ----------

fn want_mutex(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::MutexRef, Flow> {
    match v {
        Value::Mutex(m) => Ok(m.clone()),
        other => Err(i.wrong_type_mut("mutexp", other)),
    }
}

fn want_condvar(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::CondVarRef, Flow> {
    match v {
        Value::CondVar(c) => Ok(c.clone()),
        other => Err(i.wrong_type_mut("condition-variable-p", other)),
    }
}

pub(super) fn f_mutexp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Mutex(_))))
}

pub(super) fn f_make_mutex(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match a.first() {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::Mutex(std::rc::Rc::new(std::cell::RefCell::new(
        crate::lisp::value::Mutex {
            name,
            owner: None,
            count: 0,
        },
    )))
    .tracked())
}

pub(super) fn f_mutex_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    Ok(m.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

pub(super) fn f_mutex_lock(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    loop {
        let current = Value::Thread(i.threads[i.current_thread].clone());
        let available = {
            let mut mutex = m.borrow_mut();
            if mutex
                .owner
                .as_ref()
                .is_none_or(|owner| crate::lisp::builtins::eq_values(owner, &current))
            {
                mutex.owner = Some(current);
                mutex.count += 1;
                true
            } else {
                false
            }
        };
        if available {
            return Ok(Value::Nil);
        }
        crate::lisp::eval::threads::wait(
            i,
            crate::lisp::eval::threads::Wait::Mutex(m.clone()),
            Some(a[0].clone()),
        )?;
    }
}

pub(super) fn f_mutex_unlock(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    let current = Value::Thread(i.threads[i.current_thread].clone());
    let mut mutex = m.borrow_mut();
    if !mutex
        .owner
        .as_ref()
        .is_some_and(|owner| crate::lisp::builtins::eq_values(owner, &current))
    {
        drop(mutex);
        return Err(i.error("Cannot unlock mutex owned by another thread"));
    }
    mutex.count -= 1;
    if mutex.count == 0 {
        mutex.owner = None;
    }
    Ok(Value::Nil)
}

pub(super) fn f_condition_variable_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::CondVar(_))))
}

pub(super) fn f_make_condition_variable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    let name = match a.get(1) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::CondVar(std::rc::Rc::new(std::cell::RefCell::new(
        crate::lisp::value::CondVar {
            name,
            mutex: Value::Mutex(m),
            waiters: Vec::new(),
        },
    )))
    .tracked())
}

pub(super) fn f_condition_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

pub(super) fn f_condition_mutex(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow().mutex.clone())
}

fn condition_mutex_held(i: &Interp, c: &crate::lisp::value::CondVarRef) -> bool {
    let cb = c.borrow();
    let Value::Mutex(m) = &cb.mutex else {
        return false;
    };
    let mb = m.borrow();
    match &mb.owner {
        Some(Value::Thread(t)) => std::rc::Rc::ptr_eq(t, &i.threads[i.current_thread]),
        _ => false,
    }
}

pub(super) fn f_condition_wait(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error_q::<str>(
            "Condition variable's mutex is not held by current thread",
            &[],
        ));
    }
    let mutex = c.borrow().mutex.clone();
    let Value::Mutex(m) = &mutex else {
        unreachable!()
    };
    let count = {
        let mut held = m.borrow_mut();
        let count = held.count;
        held.owner = None;
        held.count = 0;
        count
    };
    let notified = std::rc::Rc::new(std::cell::Cell::new(false));
    c.borrow_mut()
        .waiters
        .push(std::rc::Rc::downgrade(&notified));
    let result = crate::lisp::eval::threads::wait(
        i,
        crate::lisp::eval::threads::Wait::Condition(notified.clone()),
        Some(a[0].clone()),
    );
    c.borrow_mut().waiters.retain(|w| {
        w.upgrade()
            .is_some_and(|w| !std::rc::Rc::ptr_eq(&w, &notified))
    });
    // A signaled waiter still reacquires its mutex before unwinding.
    f_mutex_lock(i, vec![mutex.clone()].into())?;
    m.borrow_mut().count = count;
    result?;
    Ok(Value::Nil)
}

pub(super) fn f_condition_notify(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error_q::<str>(
            "Condition variable's mutex is not held by current thread",
            &[],
        ));
    }
    let all = arg(&a, 1).truthy();
    let mut notified = false;
    c.borrow_mut().waiters.retain(|weak| {
        let Some(waiter) = weak.upgrade() else {
            return false;
        };
        if !waiter.get() && (all || !notified) {
            waiter.set(true);
            notified = true;
        }
        true
    });
    // GNU briefly releases every recursive level while notifying, then
    // reacquires the mutex before returning to Lisp.
    let mutex = c.borrow().mutex.clone();
    let Value::Mutex(m) = &mutex else {
        unreachable!()
    };
    let count = {
        let mut held = m.borrow_mut();
        let count = held.count;
        held.owner = None;
        held.count = 0;
        count
    };
    let result = crate::lisp::eval::threads::yield_now(i);
    f_mutex_lock(i, vec![mutex.clone()].into())?;
    m.borrow_mut().count = count;
    result?;
    Ok(Value::Nil)
}

/// GNU delivers a signal to the current thread synchronously, with
/// exactly the same validation and unwind behavior as `signal'.
pub(super) fn f_thread_signal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let thread = want_thread(i, &a[0])?;
    if std::rc::Rc::ptr_eq(&thread, &i.threads[i.current_thread]) {
        return crate::lisp::builtins::evalfn::f_signal(i, a[1..].to_vec().into());
    }
    if std::rc::Rc::ptr_eq(&thread, &i.threads[0]) {
        crate::lisp::eval::threads::signal_main(i, a[1].clone(), a[2].clone());
        return Ok(Value::Nil);
    }
    // Live targets receive this at their next scheduling boundary. For
    // completed targets GNU retains it for later (including repeat) joins.
    thread.borrow_mut().pending_signal = Some((a[1].clone(), a[2].clone()));
    Ok(Value::Nil)
}

pub(super) fn f_thread_buffer_disposition(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        Some(Value::Thread(_)) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("threadp", other.unwrap_or(&Value::Nil))),
    }
}

pub(super) fn f_thread_set_buffer_disposition(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        Some(Value::Thread(_)) => {}
        other => {
            return Err(i.wrong_type_mut("threadp", other.unwrap_or(&Value::Nil)));
        }
    }
    // GNU only accepts nil as the disposition.
    if !a[1].is_nil() {
        return Err(i.wrong_type_mut("null", &a[1]));
    }
    Ok(Value::Nil)
}

/// The mutex, condition variable or thread this thread is waiting for.
pub(super) fn f_thread_blocker(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(t.borrow().blocker.clone().unwrap_or(Value::Nil))
}

/// `backtrace--frames-from-thread` — threadp-checks its arg; we keep
/// no suspended frames, so nil for a real thread.
pub(super) fn f_backtrace_frames_from_thread(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_thread(i, &a[0])?;
    Ok(Value::Nil)
}
