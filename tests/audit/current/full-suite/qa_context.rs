use remacs::lisp::{Flow, Interp, OutputSink, Value};
use std::cell::RefCell;
use std::rc::Rc;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        let mut i = Interp::new();
        let output = Rc::new(RefCell::new(String::new()));
        if !args.iter().any(|a| a == "--no-capture") { i.output = Some(OutputSink::Buffer(output.clone())); }
        i.noninteractive = true;
        let source = format!("(progn (setq noninteractive t) (load {:?}) (load {:?}) nil)",
                             format!("{}/tests/qa_framework.el", std::env::current_dir().unwrap().display()), args[1]);
        match i.eval_str(&source) {
            Ok(v) => println!("RESULT {}", i.prin1_to_string(&v)),
            Err(Flow::Signal(s,d,_)) => println!("SIGNAL {} {}", i.prin1_to_string(&s), i.prin1_to_string(&d)),
            Err(e) => println!("FLOW {e:?}")
        }
        println!("CAPTURED {}", output.borrow());
    }).unwrap().join().unwrap();
}
