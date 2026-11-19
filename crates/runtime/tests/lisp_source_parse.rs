use remacs_runtime::lisp::{Interp, reader::Reader};
use std::fs;
use std::path::{Path, PathBuf};

fn collect_lisp_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read lisp directory") {
        let path = entry.expect("read lisp directory entry").path();
        if path.is_dir() {
            collect_lisp_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "el") {
            files.push(path);
        }
    }
}

#[test]
fn every_lisp_source_file_is_readable() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let lisp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../lisp");
            let mut files = Vec::new();
            collect_lisp_sources(&lisp_dir, &mut files);
            files.sort();

            let mut interp = Interp::new();
            let mut forms = 0usize;
            for path in files {
                let source = fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                let mut reader = Reader::new(&mut interp, &source);
                loop {
                    match reader.read() {
                        Ok(Some(_)) => forms += 1,
                        Ok(None) => break,
                        Err(flow) => panic!(
                            "parse {} at character {}: {flow:?}",
                            path.display(),
                            reader.position()
                        ),
                    }
                }
            }
            assert!(forms > 100_000, "unexpectedly few Lisp forms: {forms}");
        })
        .expect("spawn Lisp parse audit")
        .join()
        .expect("Lisp parse audit thread panicked");
}
