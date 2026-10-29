use remacs_runtime::{frontend::QuitRequest, lisp::Interp};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn host_quit_request_interrupts_evaluation_and_consumes_its_key() {
    let mut interp = Interp::new();
    let request = Arc::new(QuitRequest::default());
    interp.set_quit_request(Some(request.clone()));

    let input_request = request.clone();
    let input = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        input_request.request();
    });

    let result = interp.eval_str("(while t)");
    input.join().unwrap();

    assert!(result.is_err(), "the pending host quit must stop evaluation");
    assert!(
        request.discard_queued_key(),
        "the queued C-g key is consumed by the interpreter quit"
    );

    let idle_input = QuitRequest::default();
    idle_input.request();
    assert!(
        !idle_input.discard_queued_key(),
        "a C-g polled before eval must remain a normal command-loop key"
    );
}
