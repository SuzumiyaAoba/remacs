use gpui::PathPromptOptions;

pub(crate) fn path_prompt_options(
    prompt: String,
    directory: Option<String>,
    initial: Option<String>,
    directory_only: bool,
) -> PathPromptOptions {
    PathPromptOptions {
        files: !directory_only,
        directories: directory_only,
        multiple: false,
        title: Some(prompt.into()),
        prompt: None,
        directory: directory.map(std::path::PathBuf::from),
        initial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lisp_prompt_is_a_title_and_keeps_the_default_button() {
        let options = path_prompt_options("Choose a file 日本語".into(), None, None, false);
        assert_eq!(
            options.title.as_ref().map(gpui::SharedString::as_str),
            Some("Choose a file 日本語")
        );
        assert!(options.prompt.is_none());
        assert!(options.files);
        assert!(!options.directories);
        assert!(!options.multiple);
    }

    #[test]
    fn directory_and_initial_filename_survive_title_mapping() {
        let options = path_prompt_options(
            "Choose".into(),
            Some("/private/tmp/".into()),
            Some("example.el".into()),
            false,
        );
        assert_eq!(
            options.directory.as_deref(),
            Some(std::path::Path::new("/private/tmp/"))
        );
        assert_eq!(options.initial.as_deref(), Some("example.el"));
    }

    #[test]
    fn directory_only_dialog_keeps_its_selection_flags() {
        let options = path_prompt_options("Choose a directory".into(), None, None, true);
        assert!(options.directories);
        assert!(!options.files);
        assert!(!options.multiple);
        assert!(options.directory.is_none());
        assert!(options.initial.is_none());
        assert!(options.prompt.is_none());
    }
}
