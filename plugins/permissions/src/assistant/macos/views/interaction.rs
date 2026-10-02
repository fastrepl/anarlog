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
