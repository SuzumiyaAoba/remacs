use gpui::PromptButton;
use remacs_runtime::frontend::DialogItem;

pub(crate) fn dialog_prompt_items(items: &[DialogItem]) -> Vec<PromptButton> {
    let labels = items
        .iter()
        .filter_map(|item| match item {
            DialogItem::Button(label) => Some(label.clone()),
            DialogItem::Text(_) | DialogItem::Separator => None,
        })
        .collect();
    let mut buttons = super::dialogs::dialog_prompt_buttons(labels).into_iter();
    items
        .iter()
        .map(|item| match item {
            DialogItem::Button(_) => buttons.next().expect("selectable item"),
            DialogItem::Text(text) => PromptButton::text(text.clone()),
            DialogItem::Separator => PromptButton::separator(),
        })
        .collect()
}

pub(crate) fn dialog_button_index(items: &[DialogItem], index: usize) -> Option<usize> {
    if !matches!(items.get(index), Some(DialogItem::Button(_))) {
        return None;
    }
    Some(
        items[..index]
            .iter()
            .filter(|item| matches!(item, DialogItem::Button(_)))
            .count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_text_keeps_its_position_and_does_not_acquire_a_cancel_role() {
        let items = vec![
            DialogItem::Text("Cancel".into()),
            DialogItem::Button("OK".into()),
            DialogItem::Text("説明".into()),
            DialogItem::Button("Cancel".into()),
        ];
        let prompt = dialog_prompt_items(&items);
        assert!(matches!(prompt[0], PromptButton::Text(_)));
        assert!(matches!(prompt[1], PromptButton::Other(_)));
        assert!(matches!(prompt[2], PromptButton::Text(_)));
        assert!(matches!(prompt[3], PromptButton::Cancel(_)));
        assert_eq!(
            prompt
                .iter()
                .map(|item| item.label().as_str())
                .collect::<Vec<_>>(),
            ["Cancel", "OK", "説明", "Cancel"]
        );
        assert_eq!(
            (0..=4)
                .map(|index| dialog_button_index(&items, index))
                .collect::<Vec<_>>(),
            [None, Some(0), None, Some(1), None]
        );
    }

    #[test]
    fn group_boundaries_are_nonselectable_and_do_not_shift_button_values() {
        let items = [
            DialogItem::Button("Save".into()),
            DialogItem::Separator,
            DialogItem::Button("Discard".into()),
            DialogItem::Button("Cancel".into()),
        ];
        let prompt = dialog_prompt_items(&items);
        assert!(matches!(prompt[1], PromptButton::Separator(_)));
        assert!(matches!(prompt[3], PromptButton::Cancel(_)));
        assert_eq!(
            (0..=4)
                .map(|index| dialog_button_index(&items, index))
                .collect::<Vec<_>>(),
            [Some(0), None, Some(1), Some(2), None]
        );
    }
}
