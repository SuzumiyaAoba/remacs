use gpui::{PromptButton, PromptLevel};

pub(crate) fn dialog_prompt_level(header: Option<bool>) -> PromptLevel {
    match header {
        Some(false) => PromptLevel::Question,
        Some(true) => PromptLevel::Information,
        None => PromptLevel::Info,
    }
}

pub(crate) fn dialog_prompt_buttons(labels: Vec<String>) -> Vec<PromptButton> {
    // Native Cancel has one reserved ID.  Later duplicate labels must retain
    // distinct custom IDs so clicking them still returns their own index.
    let mut has_cancel = false;
    labels
        .into_iter()
        .map(|label| {
            if !has_cancel && label.eq_ignore_ascii_case("cancel") {
                has_cancel = true;
                PromptButton::cancel(label)
            } else {
                PromptButton::new(label)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_headers_select_question_or_information() {
        assert_eq!(dialog_prompt_level(Some(false)), PromptLevel::Question);
        assert_eq!(dialog_prompt_level(Some(true)), PromptLevel::Information);
        assert_eq!(dialog_prompt_level(None), PromptLevel::Info);
    }
}
