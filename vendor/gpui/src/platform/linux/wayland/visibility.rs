//! Buffer/configure lifecycle for hiding and re-mapping an xdg surface.
//! An unmap sync barrier excludes configure events from the previous mapping.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VisibilityAction {
    None,
    Draw,
    Configure,
    Unmap,
}

pub(super) struct SurfaceVisibility {
    visible: bool,
    configured: bool,
    mapped: bool,
    unmap_pending: bool,
}

impl SurfaceVisibility {
    pub(super) fn new(visible: bool) -> Self {
        Self {
            visible,
            configured: false,
            mapped: false,
            unmap_pending: false,
        }
    }

    pub(super) fn set_visible(&mut self, visible: bool) -> VisibilityAction {
        if self.visible == visible {
            return VisibilityAction::None;
        }
        self.visible = visible;
        if !visible {
            if self.mapped {
                self.mapped = false;
                self.configured = false;
                self.unmap_pending = true;
                VisibilityAction::Unmap
            } else {
                VisibilityAction::None
            }
        } else if self.unmap_pending {
            VisibilityAction::None
        } else if self.configured {
            VisibilityAction::Draw
        } else {
            VisibilityAction::Configure
        }
    }

    pub(super) fn accepts_configure(&self) -> bool {
        !self.unmap_pending
    }

    pub(super) fn remap(&mut self) -> VisibilityAction {
        let unmap = self.set_visible(false);
        let show = self.set_visible(true);
        if unmap == VisibilityAction::Unmap {
            unmap
        } else {
            show
        }
    }

    pub(super) fn configure(&mut self) -> bool {
        if !self.accepts_configure() {
            return false;
        }
        let first = !self.configured;
        self.configured = true;
        first && self.visible
    }

    pub(super) fn can_draw(&self) -> bool {
        self.visible && self.configured && !self.unmap_pending
    }

    pub(super) fn drawn(&mut self) {
        assert!(self.can_draw());
        self.mapped = true;
    }

    pub(super) fn is_visible(&self) -> bool {
        // No native map/minimize notification exists in xdg-shell. Keep the requested
        // state observable while a show request is waiting for its configure handshake.
        self.visible
    }

    pub(super) fn unmap_complete(&mut self) -> VisibilityAction {
        if !self.unmap_pending {
            return VisibilityAction::None;
        }
        self.unmap_pending = false;
        if self.visible {
            VisibilityAction::Configure
        } else {
            VisibilityAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restoring_minimized_surface_starts_a_fresh_mapping() {
        let mut surface = SurfaceVisibility::new(true);
        surface.configure();
        surface.drawn();
        assert_eq!(surface.remap(), VisibilityAction::Unmap);
        assert!(!surface.can_draw());
        assert_eq!(surface.unmap_complete(), VisibilityAction::Configure);
        assert!(surface.configure());
        surface.drawn();
    }

    #[test]
    fn hidden_initial_surface_waits_for_show_without_discarding_configure() {
        let mut surface = SurfaceVisibility::new(false);
        assert!(!surface.configure());
        assert!(!surface.can_draw());
        assert_eq!(surface.set_visible(true), VisibilityAction::Draw);
        surface.drawn();
        assert!(surface.is_visible());
    }

    #[test]
    fn hide_and_show_before_first_buffer_does_not_require_a_second_configure() {
        let mut surface = SurfaceVisibility::new(true);
        assert_eq!(surface.set_visible(false), VisibilityAction::None);
        assert!(!surface.configure());
        assert_eq!(surface.set_visible(true), VisibilityAction::Draw);
        surface.drawn();
    }

    #[test]
    fn remap_waits_for_unmap_barrier_and_a_new_configure() {
        let mut surface = SurfaceVisibility::new(true);
        assert!(surface.configure());
        surface.drawn();
        assert_eq!(surface.set_visible(false), VisibilityAction::Unmap);
        assert_eq!(surface.set_visible(true), VisibilityAction::None);
        assert!(!surface.accepts_configure());
        assert!(!surface.configure());
        assert!(!surface.can_draw());
        assert_eq!(surface.unmap_complete(), VisibilityAction::Configure);
        assert!(!surface.can_draw());
        assert!(surface.configure());
        surface.drawn();
        assert!(surface.is_visible());
    }

    #[test]
    fn a_second_hide_cancels_a_queued_remap() {
        let mut surface = SurfaceVisibility::new(true);
        surface.configure();
        surface.drawn();
        assert_eq!(surface.set_visible(false), VisibilityAction::Unmap);
        assert_eq!(surface.set_visible(true), VisibilityAction::None);
        assert_eq!(surface.set_visible(false), VisibilityAction::None);
        assert_eq!(surface.unmap_complete(), VisibilityAction::None);
        assert!(!surface.can_draw());
        assert_eq!(surface.set_visible(true), VisibilityAction::Configure);
        assert!(surface.configure());
        surface.drawn();
        assert!(surface.is_visible());
    }
}
