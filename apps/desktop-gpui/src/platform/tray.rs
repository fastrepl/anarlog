use desktop_runtime::{Result, ServiceError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    Open,
    StartMeeting,
    StopMeeting,
    Settings,
    Agenda,
    InstallUpdate,
    Hide,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayState {
    pub recording: bool,
    pub update_ready: bool,
}

impl TrayState {
    pub fn permits(&self, action: TrayAction) -> bool {
        match action {
            TrayAction::StartMeeting => !self.recording,
            TrayAction::StopMeeting => self.recording,
            TrayAction::InstallUpdate => self.update_ready && !self.recording,
            _ => true,
        }
    }
}

pub trait TrayAdapter {
    fn update(&mut self, state: &TrayState) -> Result<()>;
}

pub struct NativeTray {
    icon: tray_icon::TrayIcon,
    items: Vec<(tray_icon::menu::MenuItem, TrayAction)>,
}

impl NativeTray {
    /// Construct and poll on the GPUI main thread. Linux also requires `pump_native_events`.
    pub fn new(icon: tray_icon::Icon) -> Result<Self> {
        #[cfg(target_os = "linux")]
        gtk::init().map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        let menu = tray_icon::menu::Menu::new();
        let items = [
            ("Open Anarlog", TrayAction::Open),
            ("Start meeting", TrayAction::StartMeeting),
            ("Stop meeting", TrayAction::StopMeeting),
            ("Settings", TrayAction::Settings),
            ("Upcoming meetings", TrayAction::Agenda),
            ("Install update", TrayAction::InstallUpdate),
            ("Hide", TrayAction::Hide),
            ("Quit Anarlog", TrayAction::Quit),
        ]
        .into_iter()
        .map(|(label, action)| (tray_icon::menu::MenuItem::new(label, true, None), action))
        .collect::<Vec<_>>();
        for (item, _) in &items {
            menu.append(item)
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        }
        let icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("Anarlog")
            .build()
            .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        let mut tray = Self { icon, items };
        tray.update(&TrayState {
            recording: false,
            update_ready: false,
        })?;
        Ok(tray)
    }

    pub fn poll(&self, state: &TrayState) -> Vec<TrayAction> {
        pump_native_events();
        let mut actions = Vec::new();
        for event in tray_icon::menu::MenuEvent::receiver().try_iter().take(64) {
            if let Some((_, action)) = self.items.iter().find(|(item, _)| item.id() == &event.id)
                && state.permits(*action)
            {
                actions.push(*action);
            }
        }
        actions
    }
}

impl TrayAdapter for NativeTray {
    fn update(&mut self, state: &TrayState) -> Result<()> {
        for (item, action) in &self.items {
            item.set_enabled(state.permits(*action));
        }
        self.icon
            .set_tooltip(Some(if state.recording {
                "Anarlog — Recording"
            } else {
                "Anarlog"
            }))
            .map_err(|error| ServiceError::Failed(error.to_string().into()))
    }
}

pub fn pump_native_events() {
    #[cfg(target_os = "linux")]
    for _ in 0..64 {
        if !gtk::events_pending() {
            break;
        }
        gtk::main_iteration_do(false);
    }
}

pub struct UnavailableTray;

impl TrayAdapter for UnavailableTray {
    fn update(&mut self, _: &TrayState) -> Result<()> {
        Err(ServiceError::Unsupported(
            "Native tray host has not been attached to the GPUI event loop".into(),
        ))
    }
}
