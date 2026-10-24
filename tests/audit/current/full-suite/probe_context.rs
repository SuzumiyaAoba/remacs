use remacs::lisp::Interp;
fn main(){ std::thread::Builder::new().stack_size(64<<20).spawn(||{
 let mut i=Interp::new(); i.output=Some(remacs::lisp::OutputSink::Stdout);
 let source=std::fs::read_to_string("tests/audit/current/full-suite/probe-eval14-interactive-trace.el").unwrap();
 println!("RESULT {:?}",i.eval_str(&source));
 }).unwrap().join().unwrap(); }
