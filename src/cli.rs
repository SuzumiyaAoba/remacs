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

/// An execution view of a spelling; the original argv token is retained.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionForm<'a> {
    pub name: &'static str,
    pub value: Option<&'a str>,
}

/// Normalize supported GNU aliases and double-hyphen inline values. Do not
/// call this on operands or filenames after the standalone separator.
pub fn normalize_option(raw: &str) -> Option<OptionForm<'_>> {
    let (spelling, value) = if raw.starts_with("--") {
        match raw.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (raw, None),
        }
    } else {
        (raw, None)
    };
    let name = match spelling {
        "--eval" | "-eval" => "--eval",
        "--execute" | "-execute" => "--execute",
        "--load" | "-load" => "--load",
        "-l" => "-l",
        "--funcall" | "-funcall" => "--funcall",
        "-f" => "-f",
        "--directory" | "-directory" => "--directory",
        "-L" => "-L",
        "--script" | "-script" => "--script",
        "--dump-file" => "--dump-file",
        "--ieval" => "--ieval",
        "--batch" | "-batch" => "--batch",
        "-Q" => "-Q",
        "--quick" | "-quick" => "--quick",
        "-q" => "-q",
        "--no-init" | "--no-init-file" | "-no-init-file" => "--no-init",
        "--no-x-resources" | "-no-x-resources" => "--no-x-resources",
        "-nw" | "--no-window-system" => "-nw",
        "--version" | "-version" => "--version",
        "--help" | "-help" => "--help",
        _ => return None,
    };
    if value.is_some()
        && !matches!(
            name,
            "--eval" | "--execute" | "--load" | "--funcall" | "--directory" | "--dump-file"
        )
    {
        return None;
    }
    Some(OptionForm { name, value })
}

fn needs_operand(name: &str) -> bool {
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

/// Whether this spelling consumes the next token, rather than an inline value.
pub fn takes_operand(name: &str) -> bool {
    normalize_option(name)
        .is_some_and(|option| option.value.is_none() && needs_operand(option.name))
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
            if normalize_option(name).is_none() {
                return Some(Argument::File(name));
            }
            if takes_operand(name) {
                self.index += usize::from(self.index < self.args.len());
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
        Argument::Option { name, .. } => normalize_option(name).map(|option| option.name),
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
            let option = normalize_option(name).expect("recognized option");
            if option.name == "--dump-file" {
                let value = option
                    .value
                    .or_else(|| args.get(index + 1).map(String::as_str))
                    .ok_or("Option --dump-file requires an argument")?;
                image = Some(value.to_owned());
                removed.insert(index);
                if option.value.is_none() {
                    removed.insert(index + 1);
                }
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
            for operand in [
                "--",
                "--batch",
                "--script",
                "--version",
                "--help",
                "-help",
                "-nw",
                "--eval",
            ] {
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
        assert_eq!(options(&args).collect::<Vec<_>>(), vec!["--directory"]);
        assert_eq!(arguments(&args).last(), Some(Argument::File("file")));
    }

    #[test]
    fn aliases_and_inline_values_share_the_execution_view() {
        for (raw, canonical) in [
            ("-eval", "--eval"),
            ("-execute", "--execute"),
            ("-load", "--load"),
            ("-funcall", "--funcall"),
            ("-directory", "--directory"),
            ("-script", "--script"),
            ("-quick", "--quick"),
            ("-version", "--version"),
            ("-no-init-file", "--no-init"),
            ("--no-init-file", "--no-init"),
            ("-batch", "--batch"),
        ] {
            let form = normalize_option(raw).unwrap();
            assert_eq!(form.name, canonical);
            assert_eq!(form.value, None);
            let args = strings(&[raw, "--"]);
            assert_eq!(options(&args).next(), Some(canonical));
        }
        for raw in ["--eval=(princ \"a=b\")", "--execute=(princ \"a=b\")"] {
            assert_eq!(
                normalize_option(raw).unwrap().value,
                Some("(princ \"a=b\")")
            );
            assert!(!takes_operand(raw));
        }
        assert_eq!(normalize_option("--eval=").unwrap().value, Some(""));
        for raw in [
            "-eval=(princ 7)",
            "-execute=x",
            "-LA",
            "-no-window-system",
            "--quick=x",
        ] {
            assert!(normalize_option(raw).is_none(), "{raw}");
        }
    }

    #[test]
    fn canonical_startup_scan_keeps_raw_tokens_and_operand_boundaries() {
        let args = strings(&[
            "-load",
            "--eval=(princ 7)",
            "-eval",
            "--",
            "--directory=--batch",
            "--",
            "-version",
        ]);
        assert_eq!(
            options(&args).collect::<Vec<_>>(),
            vec!["--load", "--eval", "--directory"]
        );
        assert_eq!(arguments(&args).last(), Some(Argument::File("-version")));
        assert_eq!(args[0], "-load");
        assert_eq!(args[1], "--eval=(princ 7)");
    }

    #[test]
    fn help_is_a_startup_flag_only_outside_operands_and_separator() {
        for spelling in ["--help", "-help"] {
            assert_eq!(normalize_option(spelling).unwrap().name, "--help");
            assert!(!takes_operand(spelling));
            assert_eq!(
                options(&strings(&[spelling])).collect::<Vec<_>>(),
                vec!["--help"]
            );
            assert_eq!(options(&strings(&["--", spelling])).count(), 0);
            assert_eq!(
                options(&strings(&["--load", spelling])).collect::<Vec<_>>(),
                vec!["--load"]
            );
        }
        assert!(normalize_option("-h").is_none());
        assert!(normalize_option("--help=value").is_none());
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
