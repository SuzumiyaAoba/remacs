use accesskit::{Node, NodeId, Role, TextPosition, TextSelection, TreeId, TreeInfo, TreeUpdate};
use std::ops::Range;

const ROOT: NodeId = NodeId(0);
const BUFFER_CONTENTS: NodeId = NodeId(1);
const VISIBLE_CONTENTS: NodeId = NodeId(2);
const BUFFER_TEXT_RUN: NodeId = NodeId(3);

fn utf16_to_character_index(text: &str, offset: usize, round_up: bool) -> usize {
    let mut utf16_offset = 0;
    for (character_index, character) in text.chars().enumerate() {
        let next_offset = utf16_offset + character.len_utf16();
        if offset < next_offset {
            return if round_up && offset > utf16_offset {
                character_index + 1
            } else {
                character_index
            };
        }
        utf16_offset = next_offset;
    }
    text.chars().count()
}

pub(super) fn update(
    title: &str,
    buffer_text: &str,
    buffer_selection: Range<usize>,
    visible_text: &str,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label(title.to_owned());
    root.set_children(vec![BUFFER_CONTENTS, VISIBLE_CONTENTS]);

    let mut buffer = Node::new(Role::MultilineTextInput);
    buffer.set_label("Buffer contents");
    buffer.set_children(vec![BUFFER_TEXT_RUN]);
    buffer.set_read_only();
    buffer.set_text_selection(TextSelection {
        anchor: TextPosition {
            node: BUFFER_TEXT_RUN,
            character_index: utf16_to_character_index(buffer_text, buffer_selection.start, false),
        },
        focus: TextPosition {
            node: BUFFER_TEXT_RUN,
            character_index: utf16_to_character_index(buffer_text, buffer_selection.end, true),
        },
    });

    let mut buffer_run = Node::new(Role::TextRun);
    buffer_run.set_value(buffer_text.to_owned());
    buffer_run.set_character_lengths(
        buffer_text
            .chars()
            .map(|character| character.len_utf8() as u8)
            .collect::<Vec<_>>(),
    );

    let mut visible = Node::new(Role::Label);
    visible.set_label("Visible frame contents");
    visible.set_value(visible_text.to_owned());

    TreeUpdate {
        nodes: vec![
            (ROOT, root),
            (BUFFER_CONTENTS, buffer),
            (VISIBLE_CONTENTS, visible),
            (BUFFER_TEXT_RUN, buffer_run),
        ],
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus: BUFFER_CONTENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_buffer_and_visible_frame_text_in_the_tree() {
        let update = update(
            "Remacs",
            "buffer 😀 text",
            7..9,
            "screen\nmode line\nminibuffer",
        );
        assert_eq!(update.tree, Some(TreeInfo::new(ROOT)));
        assert_eq!(update.focus, BUFFER_CONTENTS);

        let node = |id| update.nodes.iter().find(|(node_id, _)| *node_id == id).unwrap().1.clone();
        let root = node(ROOT);
        assert_eq!(root.role(), Role::Window);
        assert_eq!(root.label(), Some("Remacs"));
        assert_eq!(root.children(), &[BUFFER_CONTENTS, VISIBLE_CONTENTS]);

        let buffer = node(BUFFER_CONTENTS);
        assert_eq!(buffer.role(), Role::MultilineTextInput);
        assert_eq!(buffer.children(), &[BUFFER_TEXT_RUN]);
        assert_eq!(
            buffer.text_selection().unwrap().anchor.character_index,
            7
        );
        assert_eq!(
            buffer.text_selection().unwrap().focus.character_index,
            8
        );

        let buffer_text = node(BUFFER_TEXT_RUN);
        assert_eq!(buffer_text.role(), Role::TextRun);
        assert_eq!(buffer_text.value(), Some("buffer 😀 text"));

        let visible = node(VISIBLE_CONTENTS);
        assert_eq!(visible.role(), Role::Label);
        assert_eq!(visible.value(), Some("screen\nmode line\nminibuffer"));
    }
}
