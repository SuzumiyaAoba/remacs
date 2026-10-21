//! Share option/operand boundaries across startup and command execution.

#[derive(Debug, PartialEq)]
pub enum Argument<'a> {
    Option { index: usize, name: &'a str },
    File(&'a str),
}

/// Recognize supported options, consuming their operands even when an
/// operand looks like an option or `--`.  Only a standalone separator
/// ends option processing; every subsequent argument is a file name.
pub fn arguments(args: &[String]) -> Arguments<'_> {
    Arguments {
        args,
        index: 0,
        options: true,
    }
}

pub struct Arguments<'a> {
    args: &'a [String],
    index: usize,
    options: bool,
}

/// Options whose next argument is an operand even when it resembles an
/// option or the standalone separator. Shared with the live Lisp dispatcher.
pub fn takes_operand(name: &str) -> bool {
    matches!(
        name,
        "--eval"
            | "--execute"
            | "--load"
            | "-l"
            | "--script"
            | "--ieval"
            | "-f"
            | "--funcall"
            | "--dump-file"
            | "-L"
            | "--directory"
    )
}

impl<'a> Iterator for Arguments<'a> {
    type Item = Argument<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.index;
            let name = self.args.get(self.index)?.as_str();
            self.index += 1;
            if !self.options {
                return Some(Argument::File(name));
            }
            if name == "--" {
                self.options = false;
                continue;
            }
            match name {
                _ if takes_operand(name) => {
                    // Keep a missing operand as an option, for the existing
                    // command handler's missing-argument behavior.
                    self.index += usize::from(self.index < self.args.len());
                }
                _ if name.starts_with("--dump-file=") => {}
                _ if name.starts_with("--directory=") => {}
                "--batch" | "-batch" | "-Q" | "--quick" | "-q" | "--no-init"
                | "-no-x-resources" | "--no-x-resources" | "-nw" | "--no-window-system"
                | "--version" => {}
                _ => return Some(Argument::File(name)),
            }
            return Some(Argument::Option {
                index: current,
                name,
            });
        }
    }
}

pub fn options(args: &[String]) -> impl Iterator<Item = &str> {
    arguments(args).filter_map(|arg| match arg {
        Argument::Option { name, .. } => Some(name),
        Argument::File(_) => None,
    })
}

/// Initial image options are consumed before Lisp command-line actions.
/// Operands and filenames following `--' retain their ordinary meaning.
pub fn dump_file(args: &[String]) -> Result<(Vec<String>, Option<String>), String> {
    let mut removed = std::collections::HashSet::new();
    let mut image = None;
    for argument in arguments(args) {
        if let Argument::Option { index, name } = argument {
            if name == "--dump-file" {
                let value = args
                    .get(index + 1)
                    .ok_or("Option --dump-file requires an argument")?;
                image = Some(value.clone());
                removed.insert(index);
                removed.insert(index + 1);
            } else if let Some(value) = name.strip_prefix("--dump-file=") {
                image = Some(value.to_owned());
                removed.insert(index);
            }
        }
    }
    Ok((
        args.iter()
            .enumerate()
            .filter(|(index, _)| !removed.contains(index))
            .map(|(_, arg)| arg.clone())
            .collect(),
        image,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn separator_preserves_every_remaining_filename() {
        let args = strings(&["-Q", "first", "--", "--eval", "(princ 123)", "--", "-Q"]);
        assert_eq!(
            arguments(&args).collect::<Vec<_>>(),
            vec![
                Argument::Option {
                    index: 0,
                    name: "-Q"
                },
                Argument::File("first"),
                Argument::File("--eval"),
                Argument::File("(princ 123)"),
                Argument::File("--"),
                Argument::File("-Q"),
            ]
        );
        assert_eq!(options(&args).collect::<Vec<_>>(), vec!["-Q"]);
    }

    #[test]
    fn option_operands_do_not_change_startup_or_end_options() {
        for option in [
            "--eval",
            "--execute",
            "--load",
            "-l",
            "--script",
            "--ieval",
            "-L",
            "--directory",
            "-f",
            "--funcall",
            "--dump-file",
        ] {
            for operand in ["--", "--batch", "--script", "--version", "-nw", "--eval"] {
                let args = strings(&[option, operand, "file", "--batch", "--", "--version"]);
                assert_eq!(
                    options(&args).collect::<Vec<_>>(),
                    vec![option, "--batch"],
                    "{option} {operand}"
                );
                assert_eq!(arguments(&args).last(), Some(Argument::File("--version")));
            }
        }
    }

    #[test]
    fn inline_directory_is_an_option_but_not_an_operand_pair() {
        let args = strings(&["--directory=--eval", "file"]);
        assert_eq!(
            options(&args).collect::<Vec<_>>(),
            vec!["--directory=--eval"]
        );
        assert_eq!(arguments(&args).last(), Some(Argument::File("file")));
    }

    #[test]
    fn mixed_repeated_options_and_files_keep_original_indices() {
        let args = strings(&[
            "a", "--eval", "one", "b", "-l", "--", "--eval", "two", "--", "-l", "c",
        ]);
        assert_eq!(
            arguments(&args).collect::<Vec<_>>(),
            vec![
                Argument::File("a"),
                Argument::Option {
                    index: 1,
                    name: "--eval"
                },
                Argument::File("b"),
                Argument::Option {
                    index: 4,
                    name: "-l"
                },
                Argument::Option {
                    index: 6,
                    name: "--eval"
                },
                Argument::File("-l"),
                Argument::File("c"),
            ]
        );
    }

    #[test]
    fn empty_trailing_separator_and_missing_operand_are_safe() {
        assert_eq!(arguments(&[]).count(), 0);
        assert_eq!(arguments(&strings(&["--"])).count(), 0);
        let args = strings(&["--batch", "--"]);
        assert_eq!(options(&args).collect::<Vec<_>>(), vec!["--batch"]);
        let args = strings(&["--eval"]);
        assert_eq!(options(&args).collect::<Vec<_>>(), vec!["--eval"]);
    }

    #[test]
    fn dump_options_are_initial_options_with_ordinary_operand_boundaries() {
        let args = strings(&[
            "--dump-file",
            "first",
            "--eval",
            "--dump-file=operand",
            "--dump-file=second",
            "--",
            "--dump-file",
            "literal",
        ]);
        assert_eq!(
            dump_file(&args).unwrap(),
            (
                strings(&[
                    "--eval",
                    "--dump-file=operand",
                    "--",
                    "--dump-file",
                    "literal"
                ]),
                Some("second".into())
            )
        );
        let args = strings(&["--dump-file", "--batch", "--eval", "x"]);
        assert_eq!(
            options(&args).collect::<Vec<_>>(),
            vec!["--dump-file", "--eval"]
        );
        assert_eq!(
            dump_file(&args).unwrap(),
            (strings(&["--eval", "x"]), Some("--batch".into()))
        );
        assert!(dump_file(&strings(&["--dump-file"])).is_err());
    }
}
