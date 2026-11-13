use accesskit::{
    ActionRequest, ActionHandler, ActivationHandler, DeactivationHandler, Node, NodeId, Role,
    TreeId, TreeInfo, TreeUpdate,
};
use std::sync::{Arc, Mutex};

fn empty_tree_update() -> TreeUpdate {
    let root = NodeId(0);
    TreeUpdate {
        nodes: vec![(root, Node::new(Role::Window))],
        tree: Some(TreeInfo::new(root)),
        tree_id: TreeId::ROOT,
        focus: root,
    }
}

struct TreeSnapshot(Arc<Mutex<TreeUpdate>>);

impl ActivationHandler for TreeSnapshot {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.0.lock().unwrap().clone())
    }
}

struct NoopActionHandler;

impl ActionHandler for NoopActionHandler {
    fn do_action(&mut self, _request: ActionRequest) {}
}

struct NoopDeactivationHandler;

impl DeactivationHandler for NoopDeactivationHandler {
    fn deactivate_accessibility(&mut self) {}
}

#[cfg(target_os = "linux")]
pub(crate) struct UnixAdapter {
    adapter: accesskit_unix::Adapter,
    tree: Arc<Mutex<TreeUpdate>>,
}

#[cfg(target_os = "linux")]
impl UnixAdapter {
    pub(crate) fn new() -> Self {
        let tree = Arc::new(Mutex::new(empty_tree_update()));
        let adapter = accesskit_unix::Adapter::new(
            TreeSnapshot(Arc::clone(&tree)),
            NoopActionHandler,
            NoopDeactivationHandler,
        );
        Self { adapter, tree }
    }

    pub(crate) fn update_tree(&mut self, tree: TreeUpdate) {
        *self.tree.lock().unwrap() = tree.clone();
        self.adapter.update_if_active(|| tree);
    }

    pub(crate) fn update_window_focus_state(&mut self, is_focused: bool) {
        self.adapter.update_window_focus_state(is_focused);
    }
}

#[cfg(target_os = "windows")]
pub(crate) struct WindowsAdapter {
    adapter: accesskit_windows::Adapter,
    tree: Arc<Mutex<TreeUpdate>>,
    activation_handler: TreeSnapshot,
}

#[cfg(target_os = "windows")]
impl WindowsAdapter {
    pub(crate) fn new(hwnd: accesskit_windows::HWND, is_focused: bool) -> Self {
        let tree = Arc::new(Mutex::new(empty_tree_update()));
        let activation_handler = TreeSnapshot(Arc::clone(&tree));
        let adapter = accesskit_windows::Adapter::new(hwnd, is_focused, NoopActionHandler);
        Self {
            adapter,
            tree,
            activation_handler,
        }
    }

    pub(crate) fn update_tree(&mut self, tree: TreeUpdate) {
        *self.tree.lock().unwrap() = tree.clone();
        if let Some(events) = self.adapter.update_if_active(|| tree) {
            events.raise();
        }
    }

    pub(crate) fn update_window_focus_state(&mut self, is_focused: bool) {
        if let Some(events) = self.adapter.update_window_focus_state(is_focused) {
            events.raise();
        }
    }

    pub(crate) fn handle_wm_getobject(&mut self, wparam: usize, lparam: isize) -> Option<isize> {
        self.adapter
            .handle_wm_getobject(
                accesskit_windows::WPARAM(wparam),
                accesskit_windows::LPARAM(lparam),
                &mut self.activation_handler,
            )
            .map(|result| {
                let result: accesskit_windows::LRESULT = result.into();
                result.0
            })
    }
}
