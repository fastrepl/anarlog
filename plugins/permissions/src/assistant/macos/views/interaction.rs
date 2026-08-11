#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DragInteraction {
    hovered: bool,
    dragging: bool,
    completed: bool,
}

impl DragInteraction {
    pub(crate) fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub(crate) fn begin_drag(&mut self) {
        self.dragging = true;
    }

    pub(crate) fn end_drag(&mut self, completed: bool) {
        self.dragging = false;
        self.completed |= completed;
    }

    pub(crate) fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub(crate) fn guide_visible(&self) -> bool {
        !self.hovered && !self.dragging && !self.completed
    }
}

#[cfg(test)]
mod tests {
    use super::DragInteraction;

    #[test]
    fn guide_visible_initially() {
        assert!(DragInteraction::default().guide_visible());
    }

    #[test]
    fn hover_hides_guide_and_exit_restores_it() {
        let mut state = DragInteraction::default();

        state.set_hovered(true);
        assert!(!state.guide_visible());

        state.set_hovered(false);
        assert!(state.guide_visible());
    }

    #[test]
    fn dragging_hides_guide() {
        let mut state = DragInteraction::default();

        state.begin_drag();

        assert!(!state.guide_visible());
    }

    #[test]
    fn canceled_drag_restores_guide_when_not_hovered() {
        let mut state = DragInteraction::default();
        state.begin_drag();

        state.end_drag(false);

        assert!(state.guide_visible());
    }

    #[test]
    fn successful_drop_keeps_guide_hidden() {
        let mut state = DragInteraction::default();
        state.begin_drag();

        state.end_drag(true);

        assert!(!state.guide_visible());
    }
}
