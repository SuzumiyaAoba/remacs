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
    entries: Vec<(WeakNativeMenuCommandId, Value)>,
}

impl NativeMenuCommands {
    pub(crate) fn register(&mut self, value: &Value) -> NativeMenuCommandId {
        for (token, old) in &self.entries {
            if eq_values(old, value)
                && let Some(token) = token.upgrade()
            {
                return token;
            }
        }
        let token = NativeMenuCommandId(Arc::new(()));
        self.entries.push((token.downgrade(), value.clone()));
        token
    }

    pub(crate) fn resolve(&self, token: &NativeMenuCommandId) -> Option<Value> {
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
        let token = commands.register(&value);
        let second_snapshot = commands.register(&value);
        assert_eq!(token, second_snapshot);
        assert!(eq_values(&commands.resolve(&token).unwrap(), &value));
        assert!(NativeMenuCommands::default().resolve(&token).is_none());
        let old_action = token.downgrade();
        drop(value);
        drop(token);
        commands.prune();
        assert!(value_lifetime.upgrade().is_some());
        drop(second_snapshot);
        commands.prune();
        assert!(old_action.upgrade().is_none());
        assert!(value_lifetime.upgrade().is_none());
    }
}
