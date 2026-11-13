use super::{GridImage, GridMenuItem};
use crate::lisp::{Value, builtins::eq_values};
use std::sync::{Arc, Weak};

/// A native menu command's lifetime token. Its Lisp value stays in the runtime.
#[derive(Clone, Debug)]
pub struct NativeMenuCommandId(Arc<()>);

impl PartialEq for NativeMenuCommandId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for NativeMenuCommandId {}

impl NativeMenuCommandId {
    /// Native action tables may outlive their menus. Keep only a weak token
    /// there; the displayed snapshot and queued input retain the command.
    pub fn downgrade(&self) -> WeakNativeMenuCommandId {
        WeakNativeMenuCommandId(Arc::downgrade(&self.0))
    }
}

/// A native action's non-owning reference to a displayed command.
#[derive(Clone, Debug)]
pub struct WeakNativeMenuCommandId(Weak<()>);

impl PartialEq for WeakNativeMenuCommandId {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for WeakNativeMenuCommandId {}

impl WeakNativeMenuCommandId {
    pub fn upgrade(&self) -> Option<NativeMenuCommandId> {
        self.0.upgrade().map(NativeMenuCommandId)
    }
}

#[derive(Default)]
pub(crate) struct NativeMenuCommands {
    entries: Vec<(WeakNativeMenuCommandId, NativeMenuBinding)>,
    tool_bars: Vec<(usize, Vec<PreparedToolBarItem>)>,
    tab_bars: Vec<(usize, Vec<PreparedTabBarItem>)>,
}

#[derive(Clone)]
pub(crate) struct NativeMenuBinding {
    pub(crate) command: Value,
    pub(crate) keys: Vec<Value>,
}

#[derive(Clone)]
pub(crate) struct PreparedToolBarItem {
    pub(crate) item: GridMenuItem,
    pub(crate) image: Option<GridImage>,
}

#[derive(Clone)]
pub(crate) struct PreparedTabBarItem {
    pub(crate) item: GridMenuItem,
    pub(crate) current: bool,
    pub(crate) button: bool,
    pub(crate) close: bool,
    pub(crate) tab_index: Option<i128>,
    pub(crate) argument: Option<i128>,
}

impl NativeMenuCommands {
    pub(crate) fn tool_bar(&self, frame: usize) -> Option<&[PreparedToolBarItem]> {
        self.tool_bars
            .iter()
            .find(|(id, _)| *id == frame)
            .map(|(_, items)| items.as_slice())
    }

    pub(crate) fn set_tool_bars(&mut self, tool_bars: Vec<(usize, Vec<PreparedToolBarItem>)>) {
        self.tool_bars = tool_bars;
    }

    pub(crate) fn tab_bar(&self, frame: usize) -> Option<&[PreparedTabBarItem]> {
        self.tab_bars
            .iter()
            .find(|(id, _)| *id == frame)
            .map(|(_, items)| items.as_slice())
    }

    pub(crate) fn set_tab_bars(&mut self, tab_bars: Vec<(usize, Vec<PreparedTabBarItem>)>) {
        self.tab_bars = tab_bars;
    }

    pub(crate) fn register(&mut self, value: &Value, keys: &[Value]) -> NativeMenuCommandId {
        for (token, old) in &self.entries {
            if eq_values(&old.command, value)
                && old.keys.len() == keys.len()
                && old.keys.iter().zip(keys).all(|(a, b)| eq_values(a, b))
                && let Some(token) = token.upgrade()
            {
                return token;
            }
        }
        let token = NativeMenuCommandId(Arc::new(()));
        self.entries.push((
            token.downgrade(),
            NativeMenuBinding {
                command: value.clone(),
                keys: keys.to_vec(),
            },
        ));
        token
    }

    pub(crate) fn resolve(&self, token: &NativeMenuCommandId) -> Option<NativeMenuBinding> {
        self.entries.iter().find_map(|(old, value)| {
            std::ptr::eq(old.0.as_ptr(), Arc::as_ptr(&token.0)).then(|| value.clone())
        })
    }

    pub(crate) fn prune(&mut self) {
        self.entries
            .retain(|(token, _)| token.0.strong_count() != 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_preserve_identity_and_release_unpublished_commands() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NativeMenuCommandId>();
        assert_send_sync::<WeakNativeMenuCommandId>();
        let mut commands = NativeMenuCommands::default();
        let value = Value::string("command object");
        let Value::Str(string) = &value else {
            unreachable!()
        };
        let value_lifetime = std::rc::Rc::downgrade(string);
        let token = commands.register(&value, &[]);
        let second_snapshot = commands.register(&value, &[]);
        assert_eq!(token, second_snapshot);
        let another_path = commands.register(&value, &[Value::Int(1)]);
        assert_ne!(token, another_path);
        assert!(eq_values(
            &commands.resolve(&token).unwrap().command,
            &value
        ));
        assert!(NativeMenuCommands::default().resolve(&token).is_none());
        let old_action = token.downgrade();
        drop(value);
        drop(token);
        commands.prune();
        assert!(value_lifetime.upgrade().is_some());
        drop(second_snapshot);
        drop(another_path);
        commands.prune();
        assert!(old_action.upgrade().is_none());
        assert!(value_lifetime.upgrade().is_none());
    }
}
