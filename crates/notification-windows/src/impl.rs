use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DRAW_TEXT_FORMAT, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE,
    DT_TOP, DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawTextW, EndPaint, FW_NORMAL, FW_SEMIBOLD,
    FillRect, HDC, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW, DI_NORMAL,
    DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, DrawIconEx, GetCursorPos,
    GetMessageW, HICON, IDC_ARROW, KillTimer, LWA_ALPHA, LoadCursorW, MF_STRING, MSG,
    PostThreadMessageW, RegisterClassExW, SPI_GETWORKAREA, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetLayeredWindowAttributes, SetTimer,
    SetWindowPos, ShowWindow, SystemParametersInfoW, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WM_APP, WM_DESTROY, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};
use windows::core::{PCWSTR, w};

use anlg_notification_interface::{
    DismissTimer, Notification, PrimaryAction, expanded_schedule_text, unix_now,
};

use crate::callbacks;
use crate::icon::{destroy_icon, header_title, load_notification_icon};
use crate::layout::{HitTarget, MAX_NOTIFICATIONS, NotificationLayout, stacked_origin};

const CLASS_NAME: windows::core::PCWSTR = w!("AnarlogNotification");
const TIMER_ID: usize = 1;

enum Command {
    Show(Box<Notification>),
    DismissAll,
}

static COMMANDS: OnceLock<Mutex<Option<Sender<Command>>>> = OnceLock::new();
static THREAD_ID: OnceLock<Mutex<u32>> = OnceLock::new();

struct NotificationInstance {
    key: String,
    payload: Notification,
    hwnd: HWND,
    layout: NotificationLayout,
    dismiss_timer: Option<DismissTimer>,
    is_hovered: bool,
    is_expanded: bool,
    tracking_leave: bool,
    icon: Option<HICON>,
}

struct NotificationManager {
    active: IndexMap<String, NotificationInstance>,
}

impl NotificationManager {
    fn new() -> Self {
        Self {
            active: IndexMap::new(),
        }
    }

    fn show(&mut self, notification: Notification) {
        let key = notification
            .key
            .clone()
            .unwrap_or_else(|| notification.title.clone());
        if let Some(existing) = self.active.shift_remove(&key) {
            unsafe {
                let _ = DestroyWindow(existing.hwnd);
            }
            destroy_icon(existing.icon);
        }
        while self.active.len() >= MAX_NOTIFICATIONS {
            if let Some(oldest) = self.active.keys().next().cloned() {
                self.dismiss(&oldest, DismissReason::Superseded);
            } else {
                break;
            }
        }

        let layout = NotificationLayout::compact(&notification);
        let occupied = self.occupied_height();
        let (work_x, work_y, work_width, _) = work_area();
        let (x, y) = stacked_origin(work_x, work_y, work_width, occupied);
        let hwnd = match create_overlay_window(x, y, layout.width, layout.height) {
            Some(hwnd) => hwnd,
            None => return,
        };
        let icon = load_notification_icon(notification.icon.as_ref());
        let timeout = notification.timeout.filter(|timeout| !timeout.is_zero());
        let instance = NotificationInstance {
            key: key.clone(),
            payload: notification,
            hwnd,
            layout,
            dismiss_timer: timeout.map(DismissTimer::new),
            is_hovered: false,
            is_expanded: false,
            tracking_leave: false,
            icon,
        };
        unsafe {
            let _ = SetTimer(Some(hwnd), TIMER_ID, 50, None);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        self.active.insert(key, instance);
        self.reposition();
    }

    fn occupied_height(&self) -> i32 {
        self.active
            .values()
            .map(|instance| instance.layout.height + crate::layout::NOTIFICATION_SPACING)
            .sum()
    }

    fn reposition(&mut self) {
        let (work_x, work_y, work_width, _) = work_area();
        let mut occupied = 0;
        for instance in self.active.values() {
            let (x, y) = stacked_origin(work_x, work_y, work_width, occupied);
            occupied += instance.layout.height + crate::layout::NOTIFICATION_SPACING;
            unsafe {
                let _ = SetWindowPos(
                    instance.hwnd,
                    None,
                    x,
                    y,
                    instance.layout.width,
                    instance.layout.height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
        }
    }

    fn dismiss(&mut self, key: &str, reason: DismissReason) {
        let Some(instance) = self.active.shift_remove(key) else {
            return;
        };
        match reason {
            DismissReason::User => callbacks::dismiss(key.to_string()),
            DismissReason::Timeout => callbacks::timeout(key.to_string()),
            DismissReason::Action | DismissReason::Superseded => {}
        }
        unsafe {
            let _ = KillTimer(Some(instance.hwnd), TIMER_ID);
            let _ = DestroyWindow(instance.hwnd);
        }
        destroy_icon(instance.icon);
        self.reposition();
    }

    fn dismiss_all(&mut self) {
        let keys: Vec<String> = self.active.keys().cloned().collect();
        for key in keys {
            self.dismiss(&key, DismissReason::Superseded);
        }
    }

    fn instance_by_hwnd_mut(&mut self, hwnd: HWND) -> Option<&mut NotificationInstance> {
        self.active
            .values_mut()
            .find(|instance| instance.hwnd == hwnd)
    }

    fn tick(&mut self, hwnd: HWND) {
        let now = Instant::now();
        let key = self.instance_by_hwnd_mut(hwnd).and_then(|instance| {
            instance
                .dismiss_timer
                .as_ref()
                .filter(|timer| timer.is_running() && timer.is_expired(now))
                .map(|_| instance.key.clone())
        });
        if let Some(key) = key {
            self.dismiss(&key, DismissReason::Timeout);
            return;
        }
        let started_key = self.instance_by_hwnd_mut(hwnd).and_then(|instance| {
            instance
                .payload
                .should_dismiss_started(unix_now())
                .then(|| instance.key.clone())
        });
        if let Some(key) = started_key {
            self.dismiss(&key, DismissReason::Timeout);
            return;
        }
        if let Some(instance) = self.instance_by_hwnd_mut(hwnd) {
            unsafe {
                let _ = InvalidateRect(Some(instance.hwnd), None, false);
            }
        }
    }

    fn set_hover(&mut self, hwnd: HWND, hovered: bool) {
        let Some(instance) = self.instance_by_hwnd_mut(hwnd) else {
            return;
        };
        if instance.is_hovered == hovered {
            return;
        }
        instance.is_hovered = hovered;
        if !instance.is_expanded
            && let Some(timer) = &mut instance.dismiss_timer
        {
            if hovered {
                timer.pause(Instant::now());
            } else {
                timer.resume(Instant::now());
            }
        }
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }

    fn handle_click(&mut self, hwnd: HWND, x: i32, y: i32) {
        let Some(instance) = self.instance_by_hwnd_mut(hwnd) else {
            return;
        };
        let target = instance.layout.action_hit(&instance.payload, x, y);
        let key = instance.key.clone();
        match target {
            HitTarget::Close => self.dismiss(&key, DismissReason::User),
            HitTarget::Action => {
                callbacks::accept(key.clone());
                self.dismiss(&key, DismissReason::Action);
            }
            HitTarget::Options => self.show_options_menu(hwnd),
            HitTarget::Footer => {
                callbacks::footer_action(key.clone());
                self.dismiss(&key, DismissReason::Action);
            }
            HitTarget::Collapse => self.toggle_expanded(hwnd),
            HitTarget::Body => {
                let Some(instance) = self.instance_by_hwnd_mut(hwnd) else {
                    return;
                };
                if instance.payload.has_options() {
                    self.show_options_menu(hwnd);
                } else if instance.payload.has_expandable_content() || instance.is_expanded {
                    self.toggle_expanded(hwnd);
                } else {
                    callbacks::confirm(key.clone());
                    self.dismiss(&key, DismissReason::Action);
                }
            }
        }
    }

    fn toggle_expanded(&mut self, hwnd: HWND) {
        let Some(instance) = self.instance_by_hwnd_mut(hwnd) else {
            return;
        };
        if !instance.payload.has_expandable_content() && !instance.is_expanded {
            return;
        }
        instance.is_expanded = !instance.is_expanded;
        instance.layout = NotificationLayout::for_state(&instance.payload, instance.is_expanded);
        if let Some(timer) = &mut instance.dismiss_timer {
            if instance.is_expanded {
                timer.pause(Instant::now());
            } else if !instance.is_hovered {
                timer.resume(Instant::now());
            }
        }
        self.reposition();
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }

    fn show_options_menu(&mut self, hwnd: HWND) {
        let Some(instance) = self.instance_by_hwnd_mut(hwnd) else {
            return;
        };
        let PrimaryAction::Options(options) = instance.payload.primary_action() else {
            return;
        };
        let options = options.to_vec();
        let key = instance.key.clone();
        let menu = unsafe { CreatePopupMenu().ok() };
        let Some(menu) = menu else {
            return;
        };
        for (index, option) in options.iter().enumerate() {
            let mut text = wide(option);
            unsafe {
                let _ = AppendMenuW(menu, MF_STRING, index + 1, PCWSTR(text.as_mut_ptr()));
            }
        }
        let mut create_new = wide("Create New Note...");
        unsafe {
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                options.len() + 1,
                PCWSTR(create_new.as_mut_ptr()),
            );
        }
        let mut cursor = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut cursor);
            let selected = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
                cursor.x,
                cursor.y,
                None,
                hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
            if selected.0 > 0 {
                callbacks::option_selected(key.clone(), selected.0 - 1);
                self.dismiss(&key, DismissReason::Action);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DismissReason {
    User,
    Timeout,
    Action,
    Superseded,
}

thread_local! {
    static MANAGER: std::cell::RefCell<NotificationManager> =
        std::cell::RefCell::new(NotificationManager::new());
}

fn ensure_ui_thread() {
    static START: std::sync::Once = std::sync::Once::new();
    START.call_once(|| {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        *COMMANDS.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(tx);
        std::thread::Builder::new()
            .name("notification-windows".into())
            .spawn(move || ui_thread(rx, ready_tx))
            .expect("failed to start Windows notification thread");
        let _ = ready_rx.recv();
    });
}

fn ui_thread(rx: Receiver<Command>, ready: Sender<()>) {
    unsafe {
        let _ = THREAD_ID
            .get_or_init(|| Mutex::new(0))
            .lock()
            .map(|mut id| *id = GetCurrentThreadId());
        register_class();
        let _ = ready.send(());
        loop {
            let mut msg = MSG::default();
            let status = GetMessageW(&mut msg, None, 0, 0);
            if status.0 <= 0 {
                break;
            }
            if msg.message == WM_APP {
                while let Ok(command) = rx.try_recv() {
                    MANAGER.with(|manager| match command {
                        Command::Show(notification) => manager.borrow_mut().show(*notification),
                        Command::DismissAll => manager.borrow_mut().dismiss_all(),
                    });
                }
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn send_command(command: Command) {
    ensure_ui_thread();
    if let Some(tx) = COMMANDS
        .get()
        .and_then(|lock| lock.lock().ok().and_then(|guard| guard.clone()))
    {
        let _ = tx.send(command);
    }
    if let Some(thread_id) = THREAD_ID
        .get()
        .and_then(|lock| lock.lock().ok().map(|id| *id))
        .filter(|id| *id != 0)
    {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_APP, WPARAM(0), LPARAM(0));
        }
    }
}

unsafe fn register_class() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        let _ = RegisterClassExW(&class);
    }
}

fn create_overlay_window(x: i32, y: i32, width: i32, height: i32) -> Option<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None).ok()?;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            CLASS_NAME,
            w!(""),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .ok()?;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 242, LWA_ALPHA);
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const std::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        Some(hwnd)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = loword(lparam);
            let y = hiword(lparam);
            MANAGER.with(|manager| manager.borrow_mut().handle_click(hwnd, x, y));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            MANAGER.with(|manager| {
                let mut manager = manager.borrow_mut();
                if let Some(instance) = manager.instance_by_hwnd_mut(hwnd)
                    && !instance.tracking_leave
                {
                    instance.tracking_leave = true;
                    let mut event = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = unsafe { TrackMouseEvent(&mut event) };
                }
                manager.set_hover(hwnd, true);
            });
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            MANAGER.with(|manager| {
                let mut manager = manager.borrow_mut();
                if let Some(instance) = manager.instance_by_hwnd_mut(hwnd) {
                    instance.tracking_leave = false;
                }
                manager.set_hover(hwnd, false);
            });
            LRESULT(0)
        }
        WM_TIMER => {
            MANAGER.with(|manager| manager.borrow_mut().tick(hwnd));
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        MANAGER.with(|manager| {
            if let Some(instance) = manager
                .borrow()
                .active
                .values()
                .find(|instance| instance.hwnd == hwnd)
            {
                draw_notification(hdc, instance);
            }
        });
        let _ = EndPaint(hwnd, &ps);
    }
}

fn draw_notification(hdc: HDC, instance: &NotificationInstance) {
    unsafe {
        let bg = CreateSolidBrush(rgb(245, 245, 247));
        let bounds = RECT {
            left: 0,
            top: 0,
            right: instance.layout.width,
            bottom: instance.layout.height,
        };
        let _ = FillRect(hdc, &bounds, bg);
        let _ = DeleteObject(bg.into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, rgb(26, 26, 26));

        let dpi = GetDpiForWindow(instance.hwnd).max(96);
        if let (Some(icon), Some(icon_rect)) = (instance.icon, instance.layout.icon) {
            let _ = DrawIconEx(
                hdc,
                icon_rect.x,
                icon_rect.y,
                icon,
                icon_rect.width,
                icon_rect.height,
                0,
                None,
                DI_NORMAL,
            );
        }

        let title_font = create_font(14, true, dpi);
        let body_font = create_font(11, false, dpi);
        let previous = SelectObject(hdc, title_font.into());
        draw_label(
            hdc,
            instance.layout.title,
            header_title(&instance.payload, instance.is_expanded),
            DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        let _ = SelectObject(hdc, body_font.into());
        SetTextColor(hdc, rgb(102, 102, 102));
        let body = if instance.is_expanded {
            expanded_body(&instance.payload, instance.schedule_remaining())
        } else {
            instance
                .payload
                .compact_message(instance.compact_remaining(Instant::now()))
        };
        draw_label(
            hdc,
            instance.layout.message,
            &body,
            if instance.is_expanded {
                DT_LEFT | DT_TOP | DT_WORDBREAK
            } else {
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS
            },
        );

        if let Some(action) = instance.layout.action {
            let (label, destructive) = match instance.payload.primary_action() {
                PrimaryAction::Options(_) if !instance.is_expanded => ("Options", false),
                PrimaryAction::Options(_) => (
                    instance.payload.expanded_action_label(),
                    instance.payload.is_destructive_action(),
                ),
                PrimaryAction::Accept { label, destructive } => (label, destructive),
            };
            let brush = CreateSolidBrush(if destructive {
                rgb(196, 43, 28)
            } else {
                rgb(230, 230, 230)
            });
            let rect = rect_from(action);
            let _ = FillRect(hdc, &rect, brush);
            let _ = DeleteObject(brush.into());
            SetTextColor(
                hdc,
                if destructive {
                    rgb(255, 255, 255)
                } else {
                    rgb(26, 26, 26)
                },
            );
            draw_label(hdc, action, label, DT_VCENTER | DT_SINGLELINE);
        }

        if let Some(collapse) = instance.layout.collapse {
            SetTextColor(hdc, rgb(26, 26, 26));
            draw_label(hdc, collapse, "Show less", DT_VCENTER | DT_SINGLELINE);
        }

        if let Some(footer) = instance.layout.footer_action
            && let Some(footer_payload) = &instance.payload.footer
        {
            SetTextColor(hdc, rgb(26, 26, 26));
            draw_label(
                hdc,
                crate::layout::Rect {
                    x: 12,
                    y: footer.y,
                    width: footer.x - 20,
                    height: footer.height,
                },
                &footer_payload.text,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
            );
            draw_label(
                hdc,
                footer,
                &footer_payload.action_label,
                DT_VCENTER | DT_SINGLELINE,
            );
        }

        if let Some(progress) = instance.layout.progress
            && let Some(timer) = &instance.dismiss_timer
        {
            let ratio = timer.progress_ratio(Instant::now()).clamp(0.0, 1.0);
            let brush = CreateSolidBrush(rgb(102, 153, 230));
            let width = ((progress.width as f64) * ratio).round() as i32;
            let rect = RECT {
                left: progress.x,
                top: progress.y,
                right: progress.x + width.max(0),
                bottom: progress.bottom(),
            };
            let _ = FillRect(hdc, &rect, brush);
            let _ = DeleteObject(brush.into());
        }

        if instance.is_hovered {
            let brush = CreateSolidBrush(rgb(255, 255, 255));
            let rect = rect_from(instance.layout.close);
            let _ = FillRect(hdc, &rect, brush);
            let _ = DeleteObject(brush.into());
            SetTextColor(hdc, rgb(0, 0, 0));
            draw_label(hdc, instance.layout.close, "×", DT_VCENTER | DT_SINGLELINE);
        }

        let _ = SelectObject(hdc, previous);
        let _ = DeleteObject(title_font.into());
        let _ = DeleteObject(body_font.into());
    }
}

fn expanded_body(notification: &Notification, remaining: Option<Duration>) -> String {
    let mut lines = Vec::new();
    if let Some(details) = &notification.event_details {
        lines.push(format!("What: {}", details.what));
        if let Some(timezone) = &details.timezone {
            lines.push(format!("Invitee Time Zone: {timezone}"));
        }
        if let Some(location) = &details.location {
            lines.push(format!("Where: {location}"));
        }
    }
    if let Some(participants) = &notification.participants {
        for participant in participants {
            let name = participant.name.clone().unwrap_or_default();
            let display = if name.is_empty() {
                participant.email.clone()
            } else {
                format!("{name} ({})", participant.email)
            };
            lines.push(display);
        }
    }
    if let Some(remaining) = remaining {
        lines.push(expanded_schedule_text(remaining));
    }
    if lines.is_empty() {
        notification.message.clone()
    } else {
        lines.join("\n")
    }
}

impl NotificationInstance {
    fn schedule_remaining(&self) -> Option<Duration> {
        let start_time = self.payload.start_time?;
        Some(Duration::from_secs(
            start_time.saturating_sub(unix_now()).max(0) as u64,
        ))
    }

    fn compact_remaining(&self, now: Instant) -> Option<Duration> {
        if self.payload.start_time.is_some() {
            self.schedule_remaining()
        } else if self.payload.shows_stop_countdown() {
            self.dismiss_timer
                .as_ref()
                .map(|timer| timer.remaining(now))
        } else {
            None
        }
    }
}

fn draw_label(hdc: HDC, rect: crate::layout::Rect, text: &str, format: DRAW_TEXT_FORMAT) {
    let mut wide_text = wide(text);
    let mut bounds = rect_from(rect);
    unsafe {
        let _ = DrawTextW(hdc, &mut wide_text, &mut bounds, format);
    }
}

fn create_font(size: i32, bold: bool, dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -((size * dpi as i32) / 96),
            0,
            0,
            0,
            if bold {
                FW_SEMIBOLD.0 as i32
            } else {
                FW_NORMAL.0 as i32
            },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    }
}

fn rect_from(rect: crate::layout::Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.right(),
        bottom: rect.bottom(),
    }
}

fn rgb(r: u32, g: u32, b: u32) -> COLORREF {
    COLORREF(r | (g << 8) | (b << 16))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn loword(lparam: LPARAM) -> i32 {
    (lparam.0 as u16) as i16 as i32
}

fn hiword(lparam: LPARAM) -> i32 {
    ((lparam.0 >> 16) as u16) as i16 as i32
}

fn work_area() -> (i32, i32, i32, i32) {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    (
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

pub fn show(notification: &Notification) {
    send_command(Command::Show(Box::new(notification.clone())));
}

pub fn dismiss_all() {
    send_command(Command::DismissAll);
}
