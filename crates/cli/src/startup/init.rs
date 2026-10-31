//! GUI initialization surrounding the shared command-line action stream.
//! Use GNU's live init loader and frame-settings implementation.

use crate::arguments::{self, Argument};
use remacs_runtime::{
    host,
    lisp::{EvalResult, Interp, Value},
};
use std::collections::HashSet;

fn get(i: &Interp, name: &str) -> Value {
    i.intern_soft(name)
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil)
}

fn set(i: &mut Interp, name: &str, value: Value) -> EvalResult {
    let symbol = host::symbol(i, name);
    i.set_symbol_default(symbol, value)?;
    Ok(Value::Nil)
}

fn call(i: &mut Interp, name: &str, args: Vec<Value>) -> EvalResult {
    let function = Value::Sym(host::symbol(i, name));
    i.apply(&function, args)
}

pub(super) fn set_init_directory(i: &mut Interp, directory: Value) -> EvalResult {
    let directory = call(i, "expand-file-name", vec![directory])?;
    let directory = call(i, "file-name-as-directory", vec![directory])?;
    set(i, "user-emacs-directory", directory.clone())?;
    set(i, "startup-init-directory", directory.clone())?;
    Ok(directory)
}

pub fn prepare_gui_startup(i: &mut Interp, args: &[String]) -> EvalResult {
    set(
        i,
        "command-line-args-left",
        Value::list(args.iter().map(Value::string).collect()),
    )?;
    if i.noninteractive
        || arguments::options(args).any(|option| matches!(option, "--batch" | "--script"))
    {
        return Ok(Value::Nil);
    }
    if let Some(frame) = i.selected_frame.clone() {
        let parameters = call(i, "frame-parameters", vec![Value::Frame(frame.clone())])?;
        set(i, "frame-initial-frame", Value::Frame(frame))?;
        set(i, "frame-initial-frame-alist", parameters)?;
        set(i, "frame-notice-user-settings", Value::t())?;
    }
    let before = call(i, "current-time", vec![])?;
    set(i, "before-init-time", before)?;
    set(i, "after-init-time", Value::Nil)?;
    set(i, "user-init-file", Value::Nil)?;

    let skip_init = arguments::options(args)
        .any(|option| matches!(option, "-Q" | "--quick" | "-q" | "--no-init"));
    if arguments::options(args)
        .any(|option| matches!(option, "-Q" | "--quick" | "--no-x-resources"))
    {
        set(i, "inhibit-x-resources", Value::t())?;
    }
    let mut explicit_directory = None;
    let mut consumed = HashSet::new();
    for argument in arguments::arguments(args) {
        if let Argument::Option { index, name } = argument {
            let option = arguments::normalize_option(name).unwrap();
            if option.name == "--init-directory" {
                let directory = option
                    .value
                    .or_else(|| args.get(index + 1).map(String::as_str))
                    .ok_or_else(|| i.error("Option --init-directory requires an argument"))?;
                explicit_directory = Some(Value::string(directory));
                consumed.insert(index);
                if option.value.is_none() {
                    consumed.insert(index + 1);
                }
            } else if matches!(
                option.name,
                "-Q" | "--quick" | "-q" | "--no-init" | "--no-x-resources"
            ) {
                consumed.insert(index);
            }
        }
    }
    set(
        i,
        "init-file-user",
        if skip_init {
            Value::Nil
        } else {
            Value::string("")
        },
    )?;
    set(
        i,
        "command-line-args-left",
        Value::list(
            args.iter()
                .enumerate()
                .filter(|(index, _)| !consumed.contains(index))
                .map(|(_, argument)| Value::string(argument))
                .collect(),
        ),
    )?;
    let xdg_base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| format!("{value}/emacs/"))
        .unwrap_or_else(|| "~/.config/emacs/".to_owned());
    let xdg = call(i, "expand-file-name", vec![Value::string(xdg_base)])?;
    set(i, "startup--xdg-config-home-emacs", xdg.clone())?;
    if let Some(directory) = explicit_directory.clone() {
        set_init_directory(i, directory)?;
    } else if !skip_init {
        let configured = get(i, "user-emacs-directory");
        let directory = if matches!(&configured, Value::Str(text) if *text.borrow() == "~/.emacs.d/")
        {
            call(i, "startup--xdg-or-homedot", vec![xdg, Value::string("")])?
        } else {
            configured
        };
        set_init_directory(i, directory)?;
    }

    let before_hook = Value::Sym(host::symbol(i, "before-init-hook"));
    call(i, "run-hooks", vec![before_hook])?;
    if !skip_init {
        // An explicit init directory is authoritative and must not execute a
        // legacy init in the caller's home directory.
        let primary = if explicit_directory.is_some() {
            i.eval_str("(lambda () nil)")?
        } else {
            i.eval_str(
                r#"(lambda ()
              (cond
               ((equal startup-init-directory startup--xdg-config-home-emacs) nil)
               ((eq system-type 'ms-dos) "~/_emacs")
               ((not (eq system-type 'windows-nt)) "~/.emacs")
               ((directory-files "~" nil "\\`\\.emacs\\(\\.elc?\\)?\\'") "~/.emacs")
               ((directory-files "~" nil "\\`_emacs\\(\\.elc?\\)?\\'") "~/_emacs")
               (t "~/.emacs")))"#,
            )?
        };
        let alternate =
            i.eval_str("(lambda () (expand-file-name \"init.el\" startup-init-directory))")?;
        call(
            i,
            "startup--load-user-init-file",
            vec![primary, alternate, Value::t()],
        )?;
    }
    let after = call(i, "current-time", vec![])?;
    set(i, "after-init-time", after)?;
    let after_hook = Value::Sym(host::symbol(i, "after-init-hook"));
    let warnings = Value::Sym(host::symbol(i, "delayed-warnings-hook"));
    call(i, "run-hooks", vec![after_hook, warnings])
}

pub fn finish_gui_startup(i: &mut Interp) -> EvalResult {
    if i.noninteractive {
        return Ok(Value::Nil);
    }
    if !get(i, "inhibit-startup-hooks").truthy() {
        let startup = Value::Sym(host::symbol(i, "emacs-startup-hook"));
        let terminal = Value::Sym(host::symbol(i, "term-setup-hook"));
        call(i, "run-hooks", vec![startup, terminal])?;
    }
    call(i, "frame-notice-user-settings", vec![])?;
    let setup = Value::Sym(host::symbol(i, "window-setup-hook"));
    call(i, "run-hooks", vec![setup])
}
