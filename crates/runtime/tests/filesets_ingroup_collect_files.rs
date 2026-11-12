use remacs_runtime::lisp::{Flow, Interp, Value};
use std::time::{SystemTime, UNIX_EPOCH};

fn collected_file_name(entry: &Value) -> String {
    let entry = entry.list_to_vec().unwrap();
    let pair = entry.first().expect("file/name pair").list_to_vec().unwrap();
    let Value::Str(file) = &pair[0] else {
        panic!("collected file name string")
    };
    file.borrow().to_unicode_lossy()
}

#[test]
fn ingroup_collection_preserves_pattern_and_match_order() {
    // Interpreter bootstrap needs more than the Rust test harness's default stack.
    std::thread::Builder::new()
        .name("filesets-ingroup-collection".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(collect_files_and_check_order)
        .unwrap()
        .join()
        .expect("filesets collection test thread panicked");
}

fn collect_files_and_check_order() {
    let mut interp = Interp::new();
    interp.eval_str("(require 'filesets)").unwrap();

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let master_path = std::env::temp_dir().join(format!(
        "filesets-ingroup-master-{}-{nonce}.txt",
        std::process::id()
    ));
    let contents = (0..8)
        .flat_map(|group| (0..8).map(move |item| (group, item)))
        .map(|(group, item)| format!("G{group:02} file-{:03}\n", group * 8 + item))
        .collect::<String>();
    std::fs::write(&master_path, contents).unwrap();

    let patterns = (0..8)
        .map(|group| {
            let name = interp
                .prin1_to_string(&Value::string(format!("Group {group:02}")))
                .to_unicode_lossy();
            let pattern = interp
                .prin1_to_string(&Value::string(format!(
                    "^G{group:02} \\(file-[0-9]+\\)$"
                )))
                .to_unicode_lossy();
            format!(
                "(list (list :name {name})\
                       (list :pattern {pattern})\
                       (list :match-number 1)\
                       (list :get-file-name (lambda (_master file) file))\
                       (list :scan-depth 0))"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let master = interp
        .prin1_to_string(&Value::string(master_path.to_string_lossy().as_ref()))
        .to_unicode_lossy();
    let form = format!(
        "(let ((filesets-ingroup-patterns\
                 (list (list \"^filesets-ingroup-master\" nil (list {patterns}))))\
               (filesets-ingroup-files nil)\
               (filesets-ingroup-cache nil))\
           (filesets-ingroup-collect-files nil nil {master}))"
    );

    let collected = interp.eval_str(&form);
    let _ = std::fs::remove_file(&master_path);
    let collected = match collected {
        Ok(collected) => collected,
        Err(Flow::Signal(signal)) => panic!(
            "collecting filesets failed: {}",
            interp.prin1_to_string(&signal.condition).to_unicode_lossy()
        ),
        Err(flow) => panic!("collecting filesets failed: {flow:?}"),
    };
    let files = collected.list_to_vec().unwrap();
    assert_eq!(files.len(), 64);
    assert_eq!(collected_file_name(&files[0]), "file-000");
    assert_eq!(collected_file_name(&files[7]), "file-007");
    assert_eq!(collected_file_name(&files[8]), "file-008");
    assert_eq!(collected_file_name(&files[63]), "file-063");
}
