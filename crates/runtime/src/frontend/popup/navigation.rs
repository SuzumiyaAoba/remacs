use super::PopupMenu;
use std::collections::HashMap;

#[derive(Default)]
pub struct PopupMenuNavigation {
    opened: Vec<usize>,
    viewports: HashMap<usize, (usize, usize)>,
    highlighted: Option<usize>,
    focus: Option<usize>,
}

pub(crate) enum PopupMenuAction {
    None,
    Redraw,
    Select(usize),
}

impl PopupMenu {
    fn subtree_end(&self, parent: usize) -> usize {
        let depth = self.entries[parent].depth;
        (parent + 1..self.entries.len())
            .find(|&index| self.entries[index].depth <= depth)
            .unwrap_or(self.entries.len())
    }

    /// Rows belonging to one level, excluding descendants of its submenus.
    pub(crate) fn level_entries(&self, parent: Option<usize>) -> Vec<usize> {
        let (mut index, end) = match parent {
            Some(parent) if self.entries.get(parent).is_some_and(|entry| entry.submenu) => {
                (parent + 1, self.subtree_end(parent))
            }
            Some(_) => return Vec::new(),
            None => (0, self.entries.len()),
        };
        let mut rows = Vec::new();
        while index < end {
            rows.push(index);
            index = if self.entries[index].submenu {
                self.subtree_end(index)
            } else {
                index + 1
            };
        }
        rows
    }

    pub(crate) fn opened_submenus(&self) -> Vec<usize> {
        self.navigation.borrow().opened.clone()
    }

    pub(crate) fn highlighted(&self) -> Option<usize> {
        self.navigation.borrow().highlighted
    }

    pub(crate) fn viewport(&self, parent: Option<usize>) -> (usize, usize) {
        match parent {
            None => (self.scroll_offset.get(), self.visible_rows.get()),
            Some(parent) => self
                .navigation
                .borrow()
                .viewports
                .get(&parent)
                .copied()
                .unwrap_or_default(),
        }
    }

    pub(crate) fn set_viewport(&self, parent: Option<usize>, offset: usize, visible: usize) {
        let offset = offset.min(
            self.level_entries(parent)
                .len()
                .saturating_sub(visible.max(1)),
        );
        match parent {
            None => {
                self.scroll_offset.set(offset);
                self.visible_rows.set(visible);
            }
            Some(parent) => {
                self.navigation
                    .borrow_mut()
                    .viewports
                    .insert(parent, (offset, visible));
            }
        }
    }

    fn column_is_open(&self, parent: Option<usize>) -> bool {
        parent.is_none_or(|parent| self.navigation.borrow().opened.contains(&parent))
    }

    fn column_of(&self, index: usize) -> Option<Option<usize>> {
        std::iter::once(None)
            .chain(self.opened_submenus().into_iter().map(Some))
            .find(|&parent| self.level_entries(parent).contains(&index))
    }

    fn visible_in(&self, parent: Option<usize>, index: usize) -> bool {
        let rows = self.level_entries(parent);
        let (offset, visible) = self.viewport(parent);
        rows.iter()
            .skip(offset)
            .take(if visible == 0 { rows.len() } else { visible })
            .any(|&row| row == index)
    }

    fn actionable(&self, index: usize) -> bool {
        self.entries
            .get(index)
            .is_some_and(|entry| entry.submenu || entry.value.is_some())
    }

    fn close_after(&self, parent: Option<usize>) {
        let keep = parent
            .and_then(|parent| {
                self.navigation
                    .borrow()
                    .opened
                    .iter()
                    .position(|&old| old == parent)
            })
            .map_or(0, |position| position + 1);
        self.navigation.borrow_mut().opened.truncate(keep);
    }

    pub(crate) fn ensure_visible(&self, parent: Option<usize>, index: usize) {
        let rows = self.level_entries(parent);
        let Some(position) = rows.iter().position(|&row| row == index) else {
            return;
        };
        let (offset, visible) = self.viewport(parent);
        let visible = visible.max(1);
        let next = if position < offset {
            position
        } else if position >= offset + visible {
            position + 1 - visible
        } else {
            offset
        };
        self.set_viewport(parent, next, visible);
    }

    fn open_submenu(&self, index: usize, focus_child: bool) -> bool {
        let Some(parent) = self.column_of(index) else {
            return false;
        };
        if !self.entries[index].submenu {
            return false;
        }
        if !self.navigation.borrow().opened.contains(&index) {
            self.close_after(parent);
            self.navigation.borrow_mut().opened.push(index);
        }
        if focus_child {
            let first = self
                .level_entries(Some(index))
                .into_iter()
                .find(|&row| self.actionable(row));
            {
                let mut state = self.navigation.borrow_mut();
                state.focus = Some(index);
                state.highlighted = first;
            }
            if let Some(first) = first {
                self.ensure_visible(Some(index), first);
            }
        } else {
            let mut state = self.navigation.borrow_mut();
            state.focus = parent;
            state.highlighted = Some(index);
        }
        true
    }

    pub(crate) fn close_submenu(&self, index: usize) -> bool {
        let Some(position) = self
            .navigation
            .borrow()
            .opened
            .iter()
            .position(|&parent| parent == index)
        else {
            return false;
        };
        let parent = self.column_of(index).flatten();
        let mut state = self.navigation.borrow_mut();
        state.opened.truncate(position);
        state.focus = parent;
        state.highlighted = Some(index);
        true
    }

    pub(crate) fn select(&self, index: usize) -> PopupMenuAction {
        let Some(parent) = self.column_of(index) else {
            return PopupMenuAction::None;
        };
        if !self.visible_in(parent, index) {
            return PopupMenuAction::None;
        }
        if self.entries[index].submenu {
            if !self.close_submenu(index) {
                self.open_submenu(index, true);
            }
            PopupMenuAction::Redraw
        } else if self.entries[index].value.is_some() {
            PopupMenuAction::Select(index)
        } else {
            PopupMenuAction::None
        }
    }

    pub(crate) fn hover(&self, parent: Option<usize>, index: Option<usize>) -> bool {
        if !self.column_is_open(parent) {
            return false;
        }
        if let Some(index) = index {
            if !self.visible_in(parent, index) {
                return false;
            }
        }
        let before = {
            let state = self.navigation.borrow();
            (state.opened.clone(), state.focus, state.highlighted)
        };
        if let Some(index) = index.filter(|&index| self.entries[index].submenu) {
            self.open_submenu(index, false);
        } else {
            self.close_after(parent);
            let mut state = self.navigation.borrow_mut();
            state.focus = parent;
            state.highlighted = index.filter(|&index| self.actionable(index));
        }
        let state = self.navigation.borrow();
        before != (state.opened.clone(), state.focus, state.highlighted)
    }

    pub(crate) fn scroll_column(&self, parent: Option<usize>, rows: isize) -> bool {
        if !self.column_is_open(parent) {
            return false;
        }
        let (offset, visible) = self.viewport(parent);
        let maximum = self
            .level_entries(parent)
            .len()
            .saturating_sub(visible.max(1));
        let next = offset.saturating_add_signed(rows).min(maximum);
        if next == offset {
            return false;
        }
        self.set_viewport(parent, next, visible);
        self.close_after(parent);
        let mut state = self.navigation.borrow_mut();
        state.focus = parent;
        state.highlighted = None;
        true
    }

    fn move_selection(&self, delta: isize, edge: bool) -> bool {
        let parent = self.navigation.borrow().focus;
        let choices = self
            .level_entries(parent)
            .into_iter()
            .filter(|&row| self.actionable(row))
            .collect::<Vec<_>>();
        if choices.is_empty() {
            return false;
        }
        let previous = self.highlighted();
        let opened = self.opened_submenus();
        let position = choices.iter().position(|&index| Some(index) == previous);
        let next = if edge {
            if delta < 0 { 0 } else { choices.len() - 1 }
        } else if let Some(position) = position {
            (position as isize + delta).rem_euclid(choices.len() as isize) as usize
        } else if delta < 0 {
            choices.len() - 1
        } else {
            0
        };
        let index = choices[next];
        self.close_after(parent);
        self.navigation.borrow_mut().highlighted = Some(index);
        self.ensure_visible(parent, index);
        previous != Some(index) || opened != self.opened_submenus()
    }

    pub(crate) fn focus_visible_levels(&self, parents: &[Option<usize>]) {
        if parents.contains(&self.navigation.borrow().focus) {
            return;
        }
        let Some(&parent) = parents.first() else {
            return;
        };
        let (offset, _) = self.viewport(parent);
        let first = self
            .level_entries(parent)
            .into_iter()
            .skip(offset)
            .find(|&row| self.actionable(row));
        let mut state = self.navigation.borrow_mut();
        state.focus = parent;
        state.highlighted = first;
    }

    pub(crate) fn key(&self, name: &str) -> PopupMenuAction {
        let changed = match name {
            "up" => self.move_selection(-1, false),
            "down" => self.move_selection(1, false),
            "home" => self.move_selection(-1, true),
            "end" => self.move_selection(1, true),
            "prior" | "next" => {
                let parent = self.navigation.borrow().focus;
                let distance = self.viewport(parent).1.max(1) as isize;
                self.move_selection(if name == "prior" { -distance } else { distance }, false)
            }
            "left" => {
                let parent = self
                    .navigation
                    .borrow()
                    .focus
                    .or_else(|| self.opened_submenus().last().copied());
                parent.is_some_and(|parent| self.close_submenu(parent))
            }
            "right" | "enter" => {
                if self.highlighted().is_none() {
                    self.move_selection(1, false);
                }
                if let Some(index) = self.highlighted() {
                    if self.entries[index].submenu {
                        self.open_submenu(index, true)
                    } else if name == "enter" && self.entries[index].value.is_some() {
                        return PopupMenuAction::Select(index);
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        if changed {
            PopupMenuAction::Redraw
        } else {
            PopupMenuAction::None
        }
    }
}
