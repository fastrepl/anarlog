use std::{collections::VecDeque, sync::Arc};

use desktop_runtime::SessionId;

pub const MAX_HISTORY: usize = 100;
pub const MAX_CLOSED: usize = 10;
const NOTE_SURFACE_MIN_WIDTH: f32 = 500.;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Route {
    Session(SessionId),
    SharedSession(Arc<str>),
    SharedPreview(Arc<str>),
    Contacts,
    Templates,
    Automations,
    Folders,
    Folder(Arc<str>),
    Human(Arc<str>),
    Organization(Arc<str>),
    Empty,
    Calendar,
    Changelog,
    Settings(Arc<str>),
    Onboarding,
    Edit(Arc<str>),
    Task(Arc<str>),
    DailySummary(Arc<str>),
}

impl Route {
    pub fn settings(section: &str) -> Self {
        match section {
            "calendar" => Self::Calendar,
            "automations" => Self::Automations,
            "folders" => Self::Folders,
            "audio" => Self::Settings("meetings".into()),
            "personalization" => Self::Settings("dictionary".into()),
            "data" => Self::Settings("imports".into()),
            "account" | "billing" | "stats" | "insights" | "app" | "meetings" | "appearance"
            | "sync" | "team" | "notifications" | "imports" | "developers" | "privacy"
            | "permissions" | "dictionary" | "dictation" | "transcription" | "intelligence"
            | "todo" => Self::Settings(section.into()),
            _ => Self::Settings("app".into()),
        }
    }

    pub fn same_resource(&self, other: &Self) -> bool {
        matches!((self, other), (Self::Settings(_), Self::Settings(_))) || self == other
    }

    pub fn custom_sidebar(&self) -> bool {
        matches!(
            self,
            Self::Contacts
                | Self::Templates
                | Self::Automations
                | Self::Folders
                | Self::Folder(_)
                | Self::Calendar
                | Self::Settings(_)
        )
    }

    pub fn pinnable(&self) -> bool {
        !matches!(self, Self::SharedSession(_) | Self::SharedPreview(_))
    }

    pub fn persistent_pin(&self) -> bool {
        self.pinnable() && !matches!(self, Self::Empty | Self::Task(_) | Self::DailySummary(_))
    }

    fn historic(&self) -> bool {
        !matches!(self, Self::Empty | Self::SharedPreview(_))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Session(_) => "Note",
            Self::SharedSession(_) => "Shared note",
            Self::SharedPreview(_) => "Shared preview",
            Self::Contacts | Self::Human(_) | Self::Organization(_) => "Contacts",
            Self::Templates => "Templates",
            Self::Automations => "Automations",
            Self::Folders => "Folders",
            Self::Folder(_) => "Folder",
            Self::Empty => "New tab",
            Self::Calendar => "Calendar",
            Self::Changelog => "What's new",
            Self::Settings(_) => "Settings",
            Self::Onboarding => "Welcome",
            Self::Edit(_) => "Edit",
            Self::Task(_) => "Task",
            Self::DailySummary(_) => "Daily summary",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SlotId(pub u64);

#[derive(Clone, Debug)]
pub struct Tab {
    pub slot: SlotId,
    pub route: Route,
    pub pinned: bool,
    pub return_to: Option<(SlotId, Route)>,
    history: VecDeque<Route>,
    cursor: usize,
}

impl Tab {
    fn push(&mut self, route: Route) {
        self.route = route.clone();
        if !route.historic() {
            return;
        }
        self.history.truncate(self.cursor + 1);
        self.history.push_back(route);
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.cursor = self.history.len().saturating_sub(1);
    }

    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }
}

#[derive(Default)]
pub struct Navigation {
    pub tabs: Vec<Tab>,
    pub active: Option<SlotId>,
    closed: VecDeque<Route>,
    next_slot: u64,
}

impl Navigation {
    pub fn current(&self) -> Option<&Tab> {
        self.tabs.iter().find(|tab| Some(tab.slot) == self.active)
    }

    pub fn open(&mut self, route: Route, new_slot: bool, protect_current: bool) -> SlotId {
        let origin = self.current().and_then(|tab| {
            (route.custom_sidebar() && !tab.route.same_resource(&route))
                .then(|| (tab.slot, tab.route.clone()))
        });
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.route.same_resource(&route))
        {
            if origin.is_some() || self.active != Some(tab.slot) {
                tab.return_to = origin;
            }
            tab.route = route.clone();
            if let Some(current) = tab.history.get_mut(tab.cursor) {
                *current = route;
            }
            self.active = Some(tab.slot);
            return tab.slot;
        }
        let replace =
            !new_slot && !protect_current && self.current().is_some_and(|tab| !tab.pinned);
        if replace {
            let slot = self.active.expect("current tab exists");
            self.clear_origins(slot);
            let tab = self.tabs.iter_mut().find(|tab| tab.slot == slot).unwrap();
            tab.return_to = origin;
            tab.push(route);
            return slot;
        }
        self.next_slot = self.next_slot.checked_add(1).expect("slot ids exhausted");
        let slot = SlotId(self.next_slot);
        let mut tab = Tab {
            slot,
            route: route.clone(),
            pinned: false,
            return_to: origin,
            history: VecDeque::new(),
            cursor: 0,
        };
        tab.push(route);
        self.tabs.push(tab);
        self.active = Some(slot);
        slot
    }

    pub fn select(&mut self, slot: SlotId) -> bool {
        if self.tabs.iter().any(|tab| tab.slot == slot) {
            self.active = Some(slot);
            true
        } else {
            false
        }
    }

    pub fn adjacent(&self, forward: bool) -> Option<SlotId> {
        let index = self
            .tabs
            .iter()
            .position(|tab| Some(tab.slot) == self.active)?;
        let index = if forward {
            (index + 1) % self.tabs.len()
        } else {
            (index + self.tabs.len() - 1) % self.tabs.len()
        };
        Some(self.tabs[index].slot)
    }

    pub fn history_target(&self, forward: bool) -> Option<Route> {
        let tab = self.current()?;
        let index = if forward {
            tab.cursor.checked_add(1)?
        } else {
            tab.cursor.checked_sub(1)?
        };
        tab.history.get(index).cloned()
    }

    pub fn travel(&mut self, forward: bool) -> bool {
        let Some(route) = self.history_target(forward) else {
            return false;
        };
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| Some(tab.slot) == self.active)
            .unwrap();
        if forward {
            tab.cursor += 1;
        } else {
            tab.cursor -= 1;
        }
        tab.route = route;
        true
    }

    pub fn close(&mut self, slot: SlotId) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.slot == slot) else {
            return false;
        };
        let tab = self.tabs.remove(index);
        if !matches!(tab.route, Route::SharedPreview(_)) {
            self.closed.push_back(tab.route);
            if self.closed.len() > MAX_CLOSED {
                self.closed.pop_front();
            }
        }
        self.clear_origins(slot);
        if self.active == Some(slot) {
            self.active = self
                .tabs
                .get(index.min(self.tabs.len().saturating_sub(1)))
                .map(|tab| tab.slot);
        }
        true
    }

    pub fn remove_sessions(&mut self, ids: &[SessionId]) {
        let slots = self
            .tabs
            .iter()
            .filter_map(|tab| {
                matches!(&tab.route, Route::Session(id) if ids.contains(id)).then_some(tab.slot)
            })
            .collect::<Vec<_>>();
        for slot in slots {
            self.close(slot);
        }
        for id in ids {
            self.invalidate(&Route::Session(id.clone()));
        }
        if self.tabs.is_empty() {
            self.open(Route::Empty, true, false);
        }
    }

    pub fn restore_target(&self) -> Option<Route> {
        self.closed.back().cloned()
    }

    pub fn restore(&mut self) -> Option<SlotId> {
        let route = self.closed.pop_back()?;
        Some(self.open(route, true, false))
    }

    pub fn pin(&mut self, slot: SlotId, pinned: bool) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.slot == slot) else {
            return false;
        };
        if !self.tabs[index].route.pinnable() || self.tabs[index].pinned == pinned {
            return false;
        }
        let mut tab = self.tabs.remove(index);
        tab.pinned = pinned;
        let boundary = self.tabs.iter().filter(|tab| tab.pinned).count();
        self.tabs.insert(boundary, tab);
        true
    }

    pub fn reorder(&mut self, slot: SlotId, at: SlotId) -> bool {
        let Some(from) = self.tabs.iter().position(|tab| tab.slot == slot) else {
            return false;
        };
        let Some(to) = self.tabs.iter().position(|tab| tab.slot == at) else {
            return false;
        };
        if self.tabs[from].pinned != self.tabs[to].pinned {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        true
    }

    pub fn invalidate(&mut self, resource: &Route) {
        for tab in &mut self.tabs {
            let before = tab
                .history
                .iter()
                .take(tab.cursor)
                .filter(|route| route.same_resource(resource))
                .count();
            tab.history.retain(|route| !route.same_resource(resource));
            tab.cursor = tab
                .cursor
                .saturating_sub(before)
                .min(tab.history.len().saturating_sub(1));
            if tab.route.same_resource(resource) {
                tab.route = if matches!(resource, Route::Session(_)) {
                    Route::Empty
                } else {
                    tab.history.get(tab.cursor).cloned().unwrap_or(Route::Empty)
                };
                tab.pinned = false;
            }
            if tab
                .return_to
                .as_ref()
                .is_some_and(|(_, route)| route.same_resource(resource))
            {
                tab.return_to = None;
            }
        }
        self.tabs.sort_by_key(|tab| !tab.pinned);
        self.closed.retain(|route| !route.same_resource(resource));
    }

    fn clear_origins(&mut self, slot: SlotId) {
        for tab in &mut self.tabs {
            if tab
                .return_to
                .as_ref()
                .is_some_and(|(origin, _)| *origin == slot)
            {
                tab.return_to = None;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SidebarState {
    width: f32,
    container_width: f32,
    proportion: Option<f32>,
    pub expanded: bool,
    saved_expanded: Option<bool>,
    note_body_width: Option<f32>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 200.,
            container_width: 0.,
            proportion: None,
            expanded: true,
            saved_expanded: None,
            note_body_width: None,
        }
    }
}

impl SidebarState {
    pub fn layout(&mut self, width: f32, route: &Route) -> Option<f32> {
        self.resize_container(width);
        if !matches!(
            route,
            Route::Session(_) | Route::SharedSession(_) | Route::SharedPreview(_) | Route::Empty
        ) || !self.expanded
        {
            self.note_body_width = None;
            return None;
        }
        let body_width = width - 4.;
        if !body_width.is_finite() || body_width <= 0. {
            return None;
        }
        self.width = self
            .width
            .min((body_width - 4. - NOTE_SURFACE_MIN_WIDTH).max(200.));
        let minimum_body_width = NOTE_SURFACE_MIN_WIDTH + 200.;
        match self.note_body_width.replace(body_width) {
            None => (body_width < minimum_body_width).then_some(minimum_body_width - body_width),
            Some(previous) if body_width < previous && body_width < minimum_body_width => {
                self.expanded = false;
                self.note_body_width = None;
                None
            }
            Some(_) => None,
        }
    }

    pub fn width(&self) -> f32 {
        if self.can_resize() { self.width } else { 200. }
    }

    pub fn can_resize(&self) -> bool {
        self.saved_expanded.is_none()
    }

    pub fn resize_container(&mut self, width: f32) {
        if width.is_finite() && width > 0. {
            self.container_width = width;
            if let Some(proportion) = self.proportion {
                self.width = (proportion * width).clamp(200., 360.);
            }
        }
    }

    pub fn set_route(&mut self, route: &Route) {
        if !matches!(
            route,
            Route::Session(_) | Route::SharedSession(_) | Route::SharedPreview(_) | Route::Empty
        ) {
            self.note_body_width = None;
        }
        if route.custom_sidebar() {
            if self.saved_expanded.is_none() {
                self.saved_expanded = Some(self.expanded);
            }
            self.expanded = true;
        } else if let Some(saved) = self.saved_expanded.take() {
            self.expanded = saved;
        }
    }

    pub fn toggle(&mut self) {
        if self.saved_expanded.is_none() {
            self.expanded = !self.expanded;
            self.note_body_width = None;
        }
    }

    pub fn resize(&mut self, width: f32) {
        if self.can_resize() && width.is_finite() {
            self.width = width.clamp(200., 360.);
            if self.container_width > 0. {
                self.proportion = Some(self.width / self.container_width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str) -> Route {
        Route::Session(SessionId(id.into()))
    }

    #[test]
    fn resource_identity_is_independent_of_slot_history() {
        let mut nav = Navigation::default();
        let first = nav.open(note("a"), true, false);
        nav.open(note("b"), false, false);
        assert_eq!(nav.current().unwrap().slot, first);
        let second = nav.open(note("c"), true, false);
        nav.open(note("d"), false, false);
        nav.select(first);
        assert!(nav.travel(false));
        assert_eq!(nav.current().unwrap().route, note("a"));
        nav.select(second);
        assert!(nav.travel(false));
        assert_eq!(nav.current().unwrap().route, note("c"));
        assert_eq!(nav.open(note("a"), true, false), first);
        assert_eq!(nav.tabs.len(), 2);
    }

    #[test]
    fn histories_and_closed_restore_are_bounded() {
        let mut nav = Navigation::default();
        let first = nav.open(note("0"), true, false);
        for index in 1..150 {
            nav.open(note(&index.to_string()), false, false);
        }
        assert_eq!(nav.current().unwrap().history.len(), MAX_HISTORY);
        nav.travel(false);
        nav.open(note("replacement"), false, false);
        assert!(!nav.current().unwrap().can_forward());
        nav.close(first);
        let restored = nav.restore().unwrap();
        assert_ne!(first, restored);
        assert_eq!(nav.current().unwrap().route, note("replacement"));
        for index in 0..30 {
            let slot = nav.open(note(&format!("closed-{index}")), true, false);
            nav.close(slot);
        }
        assert_eq!(nav.closed.len(), MAX_CLOSED);
        nav.restore();
        assert_eq!(nav.current().unwrap().route, note("closed-29"));
    }

    #[test]
    fn previews_never_enter_history_restore_or_pins() {
        let mut nav = Navigation::default();
        let slot = nav.open(note("a"), true, false);
        nav.open(Route::SharedPreview("preview".into()), false, false);
        assert_eq!(nav.current().unwrap().history.len(), 1);
        assert!(!nav.pin(slot, true));
        nav.close(slot);
        assert!(nav.restore().is_none());
        let shared = nav.open(Route::SharedSession("a".into()), true, false);
        assert!(!nav.pin(shared, true));
        assert!(!note("a").same_resource(&Route::SharedSession("a".into())));
    }

    #[test]
    fn pinned_and_recording_slots_are_not_replaced() {
        let mut nav = Navigation::default();
        let a = nav.open(note("a"), false, false);
        nav.pin(a, true);
        assert_ne!(nav.open(note("b"), false, false), a);
        let b = nav.active.unwrap();
        assert_ne!(nav.open(note("c"), false, true), b);
    }

    #[test]
    fn invalidation_and_reordering_keep_pin_groups_and_active_slot_stable() {
        let mut nav = Navigation::default();
        let a = nav.open(note("a"), true, false);
        let b = nav.open(note("b"), true, false);
        let c = nav.open(note("c"), true, false);
        nav.pin(a, true);
        nav.pin(b, true);
        assert!(!nav.reorder(c, a));
        assert!(nav.reorder(a, b));
        assert_eq!(
            nav.tabs.iter().map(|tab| tab.slot).collect::<Vec<_>>(),
            vec![b, a, c]
        );
        assert_eq!(nav.active, Some(c));
        nav.select(b);
        nav.invalidate(&note("b"));
        assert_eq!(nav.tabs[0].slot, a);
        assert!(nav.tabs[0].pinned);
        assert!(nav.tabs[1..].iter().all(|tab| !tab.pinned));
        assert_eq!(nav.active, Some(b));
        assert_eq!(nav.current().unwrap().route, Route::Empty);
    }

    #[test]
    fn deleting_active_sessions_selects_survivor_and_purges_navigation_history() {
        let mut nav = Navigation::default();
        let surviving = nav.open(note("deleted"), true, false);
        nav.open(note("survivor"), false, false);
        let removed = nav.open(note("deleted"), true, false);
        nav.pin(removed, true);
        nav.remove_sessions(&[SessionId("deleted".into())]);
        assert_eq!(nav.active, Some(surviving));
        assert_eq!(nav.current().unwrap().route, note("survivor"));
        assert_eq!(nav.history_target(false), None);
        assert_eq!(nav.restore_target(), None);
        nav.remove_sessions(&[SessionId("survivor".into())]);
        assert_eq!(nav.current().unwrap().route, Route::Empty);
        assert_eq!(nav.restore_target(), None);
    }

    #[test]
    fn invalidated_active_note_is_empty_and_cannot_be_restored() {
        let mut nav = Navigation::default();
        let slot = nav.open(note("a"), true, false);
        nav.open(note("b"), false, false);
        nav.invalidate(&note("b"));
        assert_eq!(nav.current().unwrap().route, Route::Empty);
        nav.close(slot);
        nav.restore();
        assert_ne!(nav.current().unwrap().route, note("b"));
    }

    #[test]
    fn settings_reuse_preserves_and_clears_return_origin() {
        let mut nav = Navigation::default();
        let note = nav.open(note("a"), true, false);
        let settings = nav.open(Route::settings("audio"), true, false);
        assert_eq!(nav.current().unwrap().return_to.as_ref().unwrap().0, note);
        assert_eq!(nav.open(Route::settings("data"), true, false), settings);
        assert_eq!(nav.current().unwrap().return_to.as_ref().unwrap().0, note);
        nav.close(note);
        assert!(nav.current().unwrap().return_to.is_none());
        assert_eq!(
            Route::settings("personalization"),
            Route::Settings("dictionary".into())
        );
        assert_eq!(Route::settings("folders"), Route::Folders);
        assert_eq!(Route::settings("removed"), Route::Settings("app".into()));
    }

    #[test]
    fn window_resize_preserves_expansion_for_default_and_manual_sidebars() {
        let mut sidebar = SidebarState::default();
        sidebar.resize_container(800.);
        for manual in [false, true] {
            if manual {
                sidebar.resize(280.);
            }
            for width in [708., 707., 600., 500., 800.] {
                sidebar.resize_container(width);
                assert!(sidebar.expanded);
            }
            assert_eq!(sidebar.width(), if manual { 280. } else { 200. });
            sidebar.toggle();
            for width in [500., 800.] {
                sidebar.resize_container(width);
                assert!(!sidebar.expanded);
            }
            sidebar.toggle();
        }
    }

    #[test]
    fn note_width_guard_collapses_only_after_shrinking_below_the_minimum() {
        for route in [
            note("a"),
            Route::Empty,
            Route::SharedSession("a".into()),
            Route::SharedPreview("a".into()),
        ] {
            let mut sidebar = SidebarState::default();
            assert_eq!(sidebar.layout(800., &route), None);
            assert_eq!(sidebar.layout(704., &route), None);
            assert!(sidebar.expanded);
            assert_eq!(sidebar.layout(703., &route), None);
            assert!(!sidebar.expanded);
            for width in [680., 1000., 800.] {
                assert_eq!(sidebar.layout(width, &route), None);
                assert!(!sidebar.expanded);
            }
            sidebar.toggle();
            assert_eq!(sidebar.layout(800., &route), None);
            assert!(sidebar.expanded);
        }
    }

    #[test]
    fn opening_a_narrow_note_requests_expansion_without_collapsing() {
        let mut sidebar = SidebarState::default();
        assert_eq!(sidebar.layout(600., &note("a")), Some(104.));
        assert!(sidebar.expanded);
        for width in [600., 650., 704.] {
            assert_eq!(sidebar.layout(width, &note("a")), None);
            assert!(sidebar.expanded);
        }
        sidebar.toggle();
        sidebar.layout(600., &note("a"));
        sidebar.toggle();
        assert_eq!(sidebar.layout(600., &note("a")), Some(104.));
        assert!(sidebar.expanded);
    }

    #[test]
    fn manual_sidebar_yields_to_note_minimum_without_losing_its_proportion() {
        let mut sidebar = SidebarState::default();
        let route = note("a");
        sidebar.layout(800., &route);
        sidebar.resize(280.);
        for (window_width, sidebar_width) in [
            (800., 280.),
            (1000., 350.),
            (800., 280.),
            (750., 242.),
            (708., 200.),
            (704., 200.),
            (800., 280.),
        ] {
            sidebar.layout(window_width, &route);
            assert_eq!(sidebar.width(), sidebar_width);
            assert!(sidebar.expanded);
        }
        sidebar.resize(360.);
        sidebar.layout(800., &route);
        assert_eq!(sidebar.width(), 292.);
        sidebar.layout(1000., &route);
        assert_eq!(sidebar.width(), 360.);
        sidebar.layout(708., &Route::Changelog);
        assert!(sidebar.width() > 300.);
    }

    #[test]
    fn note_width_guard_ignores_other_routes_and_invalid_measurements() {
        let mut sidebar = SidebarState::default();
        sidebar.layout(800., &note("a"));
        for width in [0., -1., f32::NAN, f32::INFINITY] {
            assert_eq!(sidebar.layout(width, &note("a")), None);
            assert!(sidebar.expanded);
        }
        for route in [
            Route::Contacts,
            Route::Changelog,
            Route::Settings("app".into()),
        ] {
            sidebar.set_route(&route);
            sidebar.layout(800., &route);
            assert_eq!(sidebar.layout(500., &route), None);
            assert!(sidebar.expanded);
        }
        sidebar.set_route(&note("a"));
        assert_eq!(sidebar.layout(600., &note("a")), Some(104.));
        assert!(sidebar.expanded);
    }

    #[test]
    fn manual_sidebar_proportion_survives_window_clamps() {
        let mut sidebar = SidebarState::default();
        sidebar.resize_container(800.);
        sidebar.resize_container(1600.);
        assert_eq!(sidebar.width(), 200.);
        sidebar.resize_container(800.);
        sidebar.resize(280.);
        sidebar.resize_container(1600.);
        assert_eq!(sidebar.width(), 360.);
        sidebar.resize_container(800.);
        assert_eq!(sidebar.width(), 280.);
        sidebar.resize_container(400.);
        assert_eq!(sidebar.width(), 200.);
        sidebar.resize_container(800.);
        assert_eq!(sidebar.width(), 280.);
        for invalid in [0., -100., f32::NAN, f32::INFINITY] {
            sidebar.resize_container(invalid);
            assert_eq!(sidebar.width(), 280.);
        }
        sidebar.resize(320.);
        sidebar.resize_container(600.);
        assert_eq!(sidebar.width(), 240.);
    }

    #[test]
    fn custom_sidebar_width_does_not_replace_timeline_proportion() {
        let mut sidebar = SidebarState::default();
        sidebar.resize_container(800.);
        sidebar.resize(280.);
        sidebar.toggle();
        sidebar.set_route(&Route::Templates);
        assert!(sidebar.expanded);
        assert!(!sidebar.can_resize());
        assert_eq!(sidebar.width(), 200.);
        sidebar.resize(350.);
        sidebar.resize_container(1600.);
        sidebar.set_route(&Route::Contacts);
        assert_eq!(sidebar.width(), 200.);
        sidebar.resize_container(800.);
        sidebar.set_route(&Route::Empty);
        assert!(!sidebar.expanded);
        assert!(sidebar.can_resize());
        sidebar.toggle();
        assert_eq!(sidebar.width(), 280.);
    }

    #[test]
    fn custom_sidebar_restores_prior_expansion_across_custom_routes() {
        let mut sidebar = SidebarState::default();
        sidebar.toggle();
        sidebar.set_route(&Route::Calendar);
        sidebar.toggle();
        assert!(sidebar.expanded);
        sidebar.set_route(&Route::Contacts);
        sidebar.set_route(&Route::Empty);
        assert!(!sidebar.expanded);
        sidebar.resize(50.);
        assert_eq!(sidebar.width, 200.);
        sidebar.resize(999.);
        assert_eq!(sidebar.width, 360.);
        sidebar.resize(f32::NAN);
        assert_eq!(sidebar.width, 360.);
    }
}
