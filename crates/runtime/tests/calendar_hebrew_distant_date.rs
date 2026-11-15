use remacs_runtime::lisp::{Interp, Value};
use std::time::{Duration, Instant};

fn distant_absolute_date_converts_in_logarithmic_year_steps() {
    let mut interp = Interp::new();
    interp.eval_str("(require 'cal-hebrew)").unwrap();

    let start = Instant::now();
    let converted = interp
        .eval_str(
            "(let* ((absolute 100000000000000)\
                    (date (calendar-hebrew-from-absolute absolute)))\
               (list (car date) (cadr date) (caddr date)\
                     (calendar-hebrew-to-absolute date)))",
        )
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "distant Hebrew conversion took {elapsed:?}"
    );
    let Value::Cons(_) = converted else {
        panic!("expected Hebrew date and round-trip absolute date, got {converted:?}");
    };
    let parts = converted.list_to_vec().unwrap();
    assert_eq!(parts.len(), 4);
    let month = parts[0].int().unwrap();
    let day = parts[1].int().unwrap();
    let year = parts[2].int().unwrap();
    assert!((1..=13).contains(&month), "month {month}");
    assert!((1..=30).contains(&day), "day {day}");
    assert!(year > 0, "year {year}");
    assert!(matches!(
        parts[3],
        Value::Int(absolute) if absolute == 100_000_000_000_000
    ));
    assert!(matches!(
        interp
            .eval_str("(calendar-hebrew-to-absolute (calendar-hebrew-from-absolute 0))")
            .unwrap(),
        Value::Int(0)
    ));
}

#[test]
fn distant_hebrew_date_round_trips_without_a_linear_year_scan() {
    std::thread::Builder::new()
        .name("calendar-hebrew-distant-date".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(distant_absolute_date_converts_in_logarithmic_year_steps)
        .unwrap()
        .join()
        .expect("distant Hebrew date test thread panicked");
}
