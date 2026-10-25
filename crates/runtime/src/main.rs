//! Standalone Lisp runner. It has no terminal or GUI dependencies.
use remacs_runtime::{
    lifecycle,
    lisp::{Flow, Interp, OutputSink, load},
};
use std::io::{self, Read};

fn main() {
    lifecycle::initialize();
    // Set before the evaluator or any worker is created.
    unsafe { std::env::set_var("REMACS_NONINTERACTIVE", "1") };
    let worker = std::thread::Builder::new()
        .name("remacs-lisp".into())
        .stack_size(512 << 20)
        .spawn(run)
        .expect("spawn Lisp evaluator");
    match worker.join() {
        Ok(code) => lifecycle::exit(code),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn run() -> i128 {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Usage: remacs-lisp [--eval EXPR | --load FILE | --script FILE]...\nWith no arguments, evaluate Lisp read from standard input."
        );
        return 0;
    }
    let mut i = Interp::new();
    i.output = Some(OutputSink::Stdout);
    let mut args = args.iter();
    let mut acted = false;
    while let Some(option) = args.next() {
        let result = match option.as_str() {
            "--eval" | "--load" | "--script" => {
                let Some(operand) = args.next() else {
                    eprintln!("remacs-lisp: {option} requires an argument");
                    return 2;
                };
                acted = true;
                match option.as_str() {
                    "--load" => load::eval_file(&mut i, operand),
                    "--script" => load::eval_file_script(&mut i, operand),
                    _ => i.eval_str(operand),
                }
            }
            _ => {
                eprintln!("remacs-lisp: unknown option {option}");
                return 2;
            }
        };
        match result {
            Ok(value) if option == "--eval" => println!("{}", i.prin1_to_string(&value)),
            Ok(_) => {}
            Err(flow) => return report(&mut i, flow),
        }
    }
    if !acted {
        let mut source = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut source) {
            eprintln!("remacs-lisp: {error}");
            return 1;
        }
        if let Err(flow) = i.eval_str(&source) {
            return report(&mut i, flow);
        }
    }
    match i.eval_str("(kill-emacs 0)") {
        Err(flow) => report(&mut i, flow),
        Ok(_) => 0,
    }
}

fn report(i: &mut Interp, flow: Flow) -> i128 {
    match flow {
        Flow::Exit(code) => code,
        Flow::Signal(signal) => {
            eprintln!("remacs-lisp: {}", i.prin1_to_string(&signal.condition));
            1
        }
        other => {
            eprintln!("remacs-lisp: {other:?}");
            1
        }
    }
}
