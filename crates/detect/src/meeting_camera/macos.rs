// Ported from Char crates/desktop-agent-automation/src/meeting_camera/macos.rs; see
// tools/camera-watermark/PROVENANCE.md.

use std::{
    thread,
    time::{Duration, Instant},
};

use anlg_overlay_kit::macos::input::{KeyModifier, can_post_key_events, post_key_chord_to_pid};
use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFArray, CFBoolean, CFRetained};

use super::{
    BackgroundEffect, BackgroundOutcome, BackgroundReading, Error, NativeMeetingApp, Outcome,
    Result,
    matching::{
        BackgroundChoice, is_camera_entry, is_camera_picker, is_webex_camera_menu,
        is_zoom_backgrounds_tab, is_zoom_blur_switch, is_zoom_settings_menu_item,
        is_zoom_video_settings, names_camera, teams_background_choice, teams_chosen_background,
        webex_entry_checked, zoom_selected_background,
    },
};
use crate::meeting_automation::{
    macos::{
        applications::{
            bring_to_front, frontmost_pid, give_front_back, is_accessibility_trusted,
            lock_meeting_app,
        },
        ax::{
            app_windows, bool_attribute, children, dom_classes, element_attribute, element_list,
            focus, is_checked, press, retain_element, role_attribute, set_attribute,
            string_attribute, windows,
        },
    },
    matching::{is_call_window, is_teams_call_marker, is_webex_call_marker},
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Web views (Teams) hold thousands of elements; Zoom's toolbar buttons list themselves among their
/// own children. A bound keeps one scan short either way.
const MAX_SCAN_NODES: usize = 6_000;
const MAX_SCAN_DEPTH: usize = 40;
/// Labels of a camera entry sit at most this far below it (Zoom: row, cell, label).
const ENTRY_LABEL_DEPTH: usize = 3;
const RETURN_KEY_CODE: u16 = 36;
const ESCAPE_KEY_CODE: u16 = 53;
const SPACE_KEY_CODE: u16 = 49;
const COMMA_KEY_CODE: u16 = 43;
/// How long the picker may take to open, or to close and show the choice.
const STEP_TIMEOUT: Duration = Duration::from_millis(2_000);

/// The call's window and the camera picker in it.
struct Call {
    window: CFRetained<AXUIElement>,
    picker: Option<CFRetained<AXUIElement>>,
}

pub(super) fn select_camera(
    app: NativeMeetingApp,
    pid: i32,
    camera: &str,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    if app == NativeMeetingApp::Slack {
        return Err(Error::UnsupportedApp);
    }
    if pid <= 0 {
        return Ok(Outcome::NoCall);
    }
    if !is_accessibility_trusted() || !can_post_key_events() {
        return Err(Error::PermissionRequired);
    }
    let _guard = lock_meeting_app();
    if cancelled() {
        return Ok(Outcome::Cancelled);
    }
    let deadline = Instant::now() + timeout;
    let application = unsafe { AXUIElement::new_application(pid) };
    let previous = frontmost_pid();
    let Some(call) = find_call_at_front(app, pid, &application, deadline)? else {
        give_front_back(previous, pid);
        return Ok(Outcome::NoCall);
    };
    let outcome = match call.picker.as_deref() {
        Some(picker) => switch(
            app,
            pid,
            &application,
            &call.window,
            picker,
            camera,
            deadline,
            cancelled,
        ),
        None => Ok(Outcome::PickerUnavailable),
    };
    give_front_back(previous, pid);
    outcome
}

#[allow(clippy::too_many_arguments)]
fn switch(
    app: NativeMeetingApp,
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    camera: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    if cancelled() {
        return Ok(Outcome::Cancelled);
    }
    if !bring_to_front(pid, application, Some(window)) {
        return Ok(Outcome::Unconfirmed);
    }
    match app {
        NativeMeetingApp::Zoom => switch_zoom(
            pid,
            application,
            window,
            picker,
            camera,
            deadline,
            cancelled,
        ),
        NativeMeetingApp::Teams => switch_teams(
            pid,
            application,
            window,
            picker,
            camera,
            deadline,
            cancelled,
        ),
        NativeMeetingApp::Webex => {
            switch_webex(pid, application, picker, camera, deadline, cancelled)
        }
        NativeMeetingApp::Slack => Err(Error::UnsupportedApp),
    }
}

pub(super) fn read_background(
    app: NativeMeetingApp,
    pid: i32,
    timeout: Duration,
) -> Result<BackgroundReading> {
    match app {
        NativeMeetingApp::Slack => return Err(Error::UnsupportedApp),
        // Webex's background tiles take no Accessibility action and show no choice.
        NativeMeetingApp::Webex => return Ok(BackgroundReading::Unsupported),
        NativeMeetingApp::Zoom | NativeMeetingApp::Teams => {}
    }
    in_call(
        app,
        pid,
        timeout,
        (BackgroundReading::NoCall, BackgroundReading::Unavailable),
        |pid, application, window, picker, deadline| match app {
            NativeMeetingApp::Zoom => Ok(
                match zoom_settings_background(pid, application, deadline)? {
                    None => BackgroundReading::Unavailable,
                    Some(BackgroundChoice::None) => BackgroundReading::Off,
                    Some(BackgroundChoice::Effect(effect)) => BackgroundReading::On(effect),
                },
            ),
            _ => teams_read_background(pid, application, window, picker, deadline),
        },
    )
}

pub(super) fn turn_off_background(
    app: NativeMeetingApp,
    pid: i32,
    known: Option<BackgroundEffect>,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<BackgroundOutcome> {
    match app {
        NativeMeetingApp::Slack => return Err(Error::UnsupportedApp),
        NativeMeetingApp::Webex => return Ok(BackgroundOutcome::Unsupported),
        NativeMeetingApp::Zoom | NativeMeetingApp::Teams => {}
    }
    in_call(
        app,
        pid,
        timeout,
        (BackgroundOutcome::NoCall, BackgroundOutcome::Unavailable),
        |pid, application, window, picker, deadline| {
            if cancelled() {
                return Ok(BackgroundOutcome::Cancelled);
            }
            match app {
                NativeMeetingApp::Zoom => zoom_background_off(
                    pid,
                    application,
                    window,
                    picker,
                    known,
                    deadline,
                    cancelled,
                ),
                _ => teams_background_off(pid, application, window, picker, deadline, cancelled),
            }
        },
    )
}

/// Runs `act` on the call in process `pid` with the app at the front, under the lock every
/// meeting-app automation takes, then gives the front back. `fallbacks` answer for no call, and
/// for a call whose picker Anarlog couldn't find or bring forward.
fn in_call<T>(
    app: NativeMeetingApp,
    pid: i32,
    timeout: Duration,
    (no_call, unavailable): (T, T),
    act: impl FnOnce(i32, &AXUIElement, &AXUIElement, &AXUIElement, Instant) -> Result<T>,
) -> Result<T> {
    if pid <= 0 {
        return Ok(no_call);
    }
    if !is_accessibility_trusted() || !can_post_key_events() {
        return Err(Error::PermissionRequired);
    }
    let _guard = lock_meeting_app();
    let deadline = Instant::now() + timeout;
    let application = unsafe { AXUIElement::new_application(pid) };
    let previous = frontmost_pid();
    let Some(call) = find_call_at_front(app, pid, &application, deadline)? else {
        give_front_back(previous, pid);
        return Ok(no_call);
    };
    let outcome = match call.picker.as_deref() {
        Some(picker) if bring_to_front(pid, &application, Some(&call.window)) => {
            act(pid, &application, &call.window, picker, deadline)
        }
        _ => Ok(unavailable),
    };
    give_front_back(previous, pid);
    outcome
}

/// Opens Teams' camera flyout (unless it is open), reads which background is in use, and closes it.
fn teams_read_background(
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    deadline: Instant,
) -> Result<BackgroundReading> {
    let Some(choices) = open_teams_backgrounds(application, window, picker, deadline)? else {
        close_teams_flyout_if_open(pid, application, window);
        return Ok(BackgroundReading::Unavailable);
    };
    close_teams_flyout(pid, application, window);
    Ok(
        match teams_chosen_background(choices.iter().map(|(choice, _, on)| (*choice, *on))) {
            Some(BackgroundChoice::None) => BackgroundReading::Off,
            Some(BackgroundChoice::Effect(effect)) => BackgroundReading::On(effect),
            None => BackgroundReading::Unavailable,
        },
    )
}

/// The background choices of Teams' camera flyout, opening it with the picker unless it is open;
/// None when it shows none in time.
fn open_teams_backgrounds(
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    deadline: Instant,
) -> Result<Option<Vec<TeamsChoice>>> {
    // The picker toggles the flyout: pressing it while it is open would close it.
    if !camera_list_open(window)? && press(picker).is_err() {
        return Ok(None);
    }
    Ok(wait_for(
        deadline.min(Instant::now() + STEP_TIMEOUT),
        || {
            teams_background_choices(application, window)
                .ok()
                .filter(|choices| !choices.is_empty())
        },
    ))
}

/// Teams lists its backgrounds under the cameras in the same flyout, as menu items whose value is
/// 1 for the one in use. Anarlog presses "No background effect" and reads it back as chosen.
fn teams_background_off(
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<BackgroundOutcome> {
    let Some(choices) = open_teams_backgrounds(application, window, picker, deadline)? else {
        close_teams_flyout_if_open(pid, application, window);
        return Ok(BackgroundOutcome::Unavailable);
    };
    let effect = match teams_chosen_background(choices.iter().map(|(choice, _, on)| (*choice, *on)))
    {
        Some(BackgroundChoice::Effect(effect)) => effect,
        Some(BackgroundChoice::None) => {
            close_teams_flyout(pid, application, window);
            return Ok(BackgroundOutcome::AlreadyOff);
        }
        None => {
            close_teams_flyout(pid, application, window);
            return Ok(BackgroundOutcome::Unavailable);
        }
    };
    let Some((_, none, _)) = choices
        .iter()
        .find(|(choice, _, _)| *choice == BackgroundChoice::None)
    else {
        close_teams_flyout(pid, application, window);
        return Ok(BackgroundOutcome::Unavailable);
    };
    if cancelled() {
        close_teams_flyout(pid, application, window);
        return Ok(BackgroundOutcome::Cancelled);
    }
    if press(none).is_err() {
        close_teams_flyout(pid, application, window);
        return Ok(BackgroundOutcome::Unavailable);
    }
    let confirmed = wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
        teams_background_choices(application, window)
            .ok()
            .filter(|choices| {
                teams_chosen_background(choices.iter().map(|(choice, _, on)| (*choice, *on)))
                    == Some(BackgroundChoice::None)
            })
            .map(|_| ())
    })
    .is_some();
    close_teams_flyout(pid, application, window);
    Ok(if confirmed {
        BackgroundOutcome::TurnedOff(effect)
    } else {
        BackgroundOutcome::Unconfirmed(effect)
    })
}

/// Teams' flyout ignores Escape while its focus is outside it, which a background press leaves it
/// in, so Anarlog focuses one of its choices first.
fn close_teams_flyout(pid: i32, application: &AXUIElement, window: &AXUIElement) {
    if let Ok(choices) = teams_background_choices(application, window)
        && let Some((_, element, _)) = choices.first()
    {
        let _ = focus(element, pid);
        thread::sleep(POLL_INTERVAL);
    }
    close_with_escape(pid);
}

/// Escape in a call with no flyout open could leave full screen, so only an open flyout gets it.
fn close_teams_flyout_if_open(pid: i32, application: &AXUIElement, window: &AXUIElement) {
    if camera_list_open(window).unwrap_or(false) {
        close_teams_flyout(pid, application, window);
    }
}

/// A background choice in Teams' flyout, its element, and whether it is in use.
type TeamsChoice = (BackgroundChoice, CFRetained<AXUIElement>, bool);

/// The background choices of Teams' open flyout, each with whether it is in use.
fn teams_background_choices(
    application: &AXUIElement,
    window: &AXUIElement,
) -> Result<Vec<TeamsChoice>> {
    let mut roots = vec![retain_element(window)];
    for other in windows(application)? {
        if *other != *window {
            roots.push(other);
        }
    }
    let mut found = Vec::new();
    for root in roots {
        let mut stack = vec![(root, 0_usize)];
        let mut visited = 0_usize;
        while let Some((element, depth)) = stack.pop() {
            visited += 1;
            if visited > MAX_SCAN_NODES {
                break;
            }
            let role = role_attribute(&element)?;
            if let Some(choice) = teams_background_choice(&role, &labels_of(&element)?) {
                let on = is_checked(&element)?;
                found.push((choice, element, on));
                continue;
            }
            if depth < MAX_SCAN_DEPTH {
                for child in children(&element)? {
                    stack.push((child, depth + 1));
                }
            }
        }
        if !found.is_empty() {
            break;
        }
    }
    Ok(found)
}

/// Zoom's camera menu shows only whether blur is on; a background image shows only in Settings,
/// as the Backgrounds tile read "Selected". Anarlog reads Settings (unless `known` says what it
/// just read there), then clears the effect with the menu's "Blur my background" switch: switching
/// blur off leaves Zoom on None, whatever was chosen before, so an image takes blur on, then off.
/// Cancellation is honoured only before the first press: after it, the app's effect may already
/// differ, so the turn-off finishes.
#[allow(clippy::too_many_arguments)]
fn zoom_background_off(
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    known: Option<BackgroundEffect>,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<BackgroundOutcome> {
    let effect = match known {
        Some(effect) => effect,
        None => match zoom_settings_background(pid, application, deadline)? {
            None => return Ok(BackgroundOutcome::Unavailable),
            Some(BackgroundChoice::None) => return Ok(BackgroundOutcome::AlreadyOff),
            Some(BackgroundChoice::Effect(effect)) => effect,
        },
    };
    // Until the blur switch is pressed nothing about the effect has changed: those failures are
    // `Unavailable`, never `Unconfirmed`.
    if !bring_to_front(pid, application, Some(window)) {
        return Ok(BackgroundOutcome::Unavailable);
    }
    // Zoom lists its windows only once it is active; a press before that opens nothing.
    let Some(before) = wait_for(deadline, || {
        windows(application)
            .ok()
            .filter(|listed| listed.iter().any(|listed| **listed == *window))
    }) else {
        return Ok(BackgroundOutcome::Unavailable);
    };
    if press(picker).is_err() {
        return Ok(BackgroundOutcome::Unavailable);
    }
    let Some(menu) = wait_for(deadline, || new_window_with_table(application, &before)) else {
        return Ok(BackgroundOutcome::Unavailable);
    };
    let blur_on = |menu: &AXUIElement| -> Option<(CFRetained<AXUIElement>, bool)> {
        let switch = descendant_where(menu, is_zoom_blur_switch).ok().flatten()?;
        let on = string_attribute(&switch, "AXValue")
            .ok()
            .flatten()
            .is_some_and(|value| value.eq_ignore_ascii_case("on"));
        Some((switch, on))
    };
    let Some((switch, on)) = blur_on(&menu) else {
        close_with_escape(pid);
        return Ok(BackgroundOutcome::Unavailable);
    };
    if cancelled() {
        close_with_escape(pid);
        return Ok(BackgroundOutcome::Cancelled);
    }
    let presses = if on { 1 } else { 2 };
    let mut expect_on = on;
    for _ in 0..presses {
        expect_on = !expect_on;
        let flipped = press(&switch).is_ok()
            && wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
                blur_on(&menu)
                    .filter(|(_, on)| *on == expect_on)
                    .map(|_| ())
            })
            .is_some();
        if !flipped {
            close_with_escape(pid);
            return Ok(BackgroundOutcome::Unconfirmed(effect));
        }
    }
    close_with_escape(pid);
    Ok(BackgroundOutcome::TurnedOff(effect))
}

/// What Zoom Settings shows as the chosen background, or None when Anarlog couldn't read it. Opens
/// Settings from Zoom's app menu (⌘, posted to Zoom is sometimes ignored) on its Video & effects
/// page, Backgrounds tab, and closes it again unless the person had it open.
fn zoom_settings_background(
    pid: i32,
    application: &AXUIElement,
    deadline: Instant,
) -> Result<Option<BackgroundChoice>> {
    let was_open = zoom_settings_window(application)?.is_some();
    if !was_open
        && !press_app_menu_item(application, is_zoom_settings_menu_item)?
        && !post_key_chord_to_pid(pid, COMMA_KEY_CODE, &[KeyModifier::Command])
    {
        return Ok(None);
    }
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    let Some(settings) = wait_for(step, || zoom_settings_window(application).ok().flatten()) else {
        return Ok(None);
    };
    let choice = read_zoom_backgrounds(&settings, deadline);
    if !was_open && let Some(close) = element_attribute(&settings, "AXCloseButton")? {
        let _ = press(&close);
    }
    choice
}

fn read_zoom_backgrounds(
    settings: &AXUIElement,
    deadline: Instant,
) -> Result<Option<BackgroundChoice>> {
    if let Some(category) = descendant_where(settings, |role, _| role == "AXTable")? {
        for row in element_list(&category, "AXRows")? {
            if labels_below(&row, ENTRY_LABEL_DEPTH)?
                .iter()
                .any(|label| is_zoom_video_settings(label))
            {
                let selection = CFArray::from_objects(&[&*row]);
                set_attribute(&category, "AXSelectedRows", selection.as_ref());
                break;
            }
        }
    }
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    let Some(tab) = wait_for(step, || {
        descendant_where(settings, |_, labels| {
            labels
                .first()
                .is_some_and(|title| is_zoom_backgrounds_tab(title))
        })
        .ok()
        .flatten()
    }) else {
        return Ok(None);
    };
    let _ = press(&tab);
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    Ok(wait_for(step, || zoom_chosen_tile(settings).ok().flatten()))
}

/// The chosen tile of Zoom's Backgrounds list.
fn zoom_chosen_tile(settings: &AXUIElement) -> Result<Option<BackgroundChoice>> {
    let mut stack = vec![(retain_element(settings), 0_usize)];
    let mut visited = 0_usize;
    while let Some((element, depth)) = stack.pop() {
        visited += 1;
        if visited > MAX_SCAN_NODES {
            break;
        }
        if let Some(identifier) = string_attribute(&element, "AXIdentifier")?
            && let Some(choice) = zoom_selected_background(
                &identifier,
                &string_attribute(&element, "AXDescription")?.unwrap_or_default(),
                &string_attribute(&element, "AXValueDescription")?.unwrap_or_default(),
            )
        {
            return Ok(Some(choice));
        }
        if depth < MAX_SCAN_DEPTH {
            for child in children(&element)? {
                stack.push((child, depth + 1));
            }
        }
    }
    Ok(None)
}

/// Presses the item of the app's own menu (the one named after the app) whose title matches.
fn press_app_menu_item(application: &AXUIElement, matches: impl Fn(&str) -> bool) -> Result<bool> {
    let Some(bar) = element_attribute(application, "AXMenuBar")? else {
        return Ok(false);
    };
    // The first menu is Apple's; the app's own menu follows it.
    let Some(app_menu) = children(&bar)?.into_iter().nth(1) else {
        return Ok(false);
    };
    for menu in children(&app_menu)? {
        for item in children(&menu)? {
            if string_attribute(&item, "AXTitle")?.is_some_and(|title| matches(&title)) {
                return Ok(press(&item).is_ok());
            }
        }
    }
    Ok(false)
}

/// Zoom's Settings window, which it lists as its focused window while it is the front app.
fn zoom_settings_window(application: &AXUIElement) -> Result<Option<CFRetained<AXUIElement>>> {
    let mut candidates = windows(application)?;
    if let Some(focused) = element_attribute(application, "AXFocusedWindow")? {
        candidates.push(focused);
    }
    for window in candidates {
        if string_attribute(&window, "AXTitle")?.as_deref() == Some("Settings") {
            return Ok(Some(window));
        }
    }
    Ok(None)
}

/// The first element below `root` whose role and labels (title first) match.
fn descendant_where(
    root: &AXUIElement,
    matches: impl Fn(&str, &[String]) -> bool,
) -> Result<Option<CFRetained<AXUIElement>>> {
    let mut stack = vec![(retain_element(root), 0_usize)];
    let mut visited = 0_usize;
    while let Some((element, depth)) = stack.pop() {
        visited += 1;
        if visited > MAX_SCAN_NODES {
            break;
        }
        if matches(&role_attribute(&element)?, &labels_of(&element)?) {
            return Ok(Some(element));
        }
        if depth < MAX_SCAN_DEPTH {
            for child in children(&element)?.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
    Ok(None)
}

/// Zoom draws its camera list as a table in a window of its own ("Menu window"). Its rows take no
/// press and show no checkmark to Accessibility, so Anarlog selects the row and presses Return, as
/// the keyboard does, and counts the camera as chosen when the menu closes on that row.
fn switch_zoom(
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    camera: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    // Zoom lists its windows only once it is active; a press before that opens nothing.
    let Some(before) = wait_for(deadline, || {
        windows(application)
            .ok()
            .filter(|listed| listed.iter().any(|listed| **listed == *window))
    }) else {
        return Ok(Outcome::Unconfirmed);
    };
    if press(picker).is_err() {
        return Ok(Outcome::PickerUnavailable);
    }
    let Some(menu) = wait_for(deadline, || new_window_with_table(application, &before)) else {
        return Ok(Outcome::PickerUnavailable);
    };
    let Some(table) = first_descendant(&menu, |role| role == "AXTable")? else {
        close_with_escape(pid);
        return Ok(Outcome::PickerUnavailable);
    };
    let rows = element_list(&table, "AXRows")?;
    let mut target = None;
    for row in rows {
        if names_camera(camera, &labels_below(&row, ENTRY_LABEL_DEPTH)?) {
            target = Some(row);
            break;
        }
    }
    let Some(row) = target else {
        close_with_escape(pid);
        return Ok(Outcome::CameraMissing);
    };
    let selection = CFArray::from_objects(&[&*row]);
    set_attribute(&table, "AXSelectedRows", selection.as_ref());
    let selected = element_list(&table, "AXSelectedRows")?;
    // Return goes only to Zoom at the front, with the menu open on exactly this camera.
    let on_target = selected.len() == 1 && *selected[0] == *row;
    if !on_target || frontmost_pid() != Some(pid) || !is_open(application, &menu)? {
        close_with_escape(pid);
        return Ok(Outcome::Unconfirmed);
    }
    if cancelled() {
        close_with_escape(pid);
        return Ok(Outcome::Cancelled);
    }
    if !post_key_chord_to_pid(pid, RETURN_KEY_CODE, &[]) {
        close_with_escape(pid);
        return Ok(Outcome::Unconfirmed);
    }
    if wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
        (!is_open(application, &menu).unwrap_or(true)).then_some(())
    })
    .is_some()
    {
        Ok(Outcome::Switched)
    } else {
        close_with_escape(pid);
        Ok(Outcome::Unconfirmed)
    }
}

/// Teams opens a flyout listing the cameras in the call's web view. Anarlog presses the camera's
/// entry, then reads the entry back as checked, opening the flyout again if pressing closed it.
fn switch_teams(
    pid: i32,
    application: &AXUIElement,
    window: &AXUIElement,
    picker: &AXUIElement,
    camera: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    if press(picker).is_err() {
        return Ok(Outcome::PickerUnavailable);
    }
    // The flyout lists every camera at once: once it is open, a camera it doesn't show is missing.
    if wait_for(deadline, || {
        camera_list_open(window).ok().filter(|open| *open)
    })
    .is_none()
    {
        close_with_escape(pid);
        return Ok(Outcome::PickerUnavailable);
    }
    let Some(entry) = wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
        camera_entry(application, window, camera).ok().flatten()
    }) else {
        close_with_escape(pid);
        return Ok(Outcome::CameraMissing);
    };
    if is_checked(&entry)? {
        close_with_escape(pid);
        return Ok(Outcome::Switched);
    }
    if cancelled() {
        close_with_escape(pid);
        return Ok(Outcome::Cancelled);
    }
    if press(&entry).is_err() {
        close_with_escape(pid);
        return Ok(Outcome::Unconfirmed);
    }
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    let mut confirmed = wait_for(step, || {
        camera_entry(application, window, camera)
            .ok()
            .flatten()
            .filter(|entry| is_checked(entry).unwrap_or(false))
            .map(|_| ())
    })
    .is_some();
    // Choosing closed the flyout: open it once more to read the choice, then close it again.
    if !confirmed && camera_entry(application, window, camera)?.is_none() && press(picker).is_ok() {
        let reopened = wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
            camera_entry(application, window, camera).ok().flatten()
        });
        confirmed = reopened.is_some_and(|entry| is_checked(&entry).unwrap_or(false));
    }
    if camera_entry(application, window, camera)?.is_some() {
        close_with_escape(pid);
    }
    Ok(if confirmed {
        Outcome::Switched
    } else {
        Outcome::Unconfirmed
    })
}

/// Webex lists its cameras in a popover window whose rows take no press, so Anarlog selects the
/// camera's row and presses Space, as the keyboard does. Choosing closes the list; Anarlog opens it
/// once more to read the row back as checked, then closes it with the same button.
fn switch_webex(
    pid: i32,
    application: &AXUIElement,
    picker: &AXUIElement,
    camera: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    if press(picker).is_err() {
        return Ok(Outcome::PickerUnavailable);
    }
    let Some(menu) = wait_for(deadline, || webex_camera_menu(application).ok().flatten()) else {
        return Ok(Outcome::PickerUnavailable);
    };
    let Some(row) = webex_camera_row(&menu, camera)? else {
        close_webex_menu(application, picker);
        return Ok(Outcome::CameraMissing);
    };
    if webex_entry_checked(camera, &labels_below(&row, ENTRY_LABEL_DEPTH)?) {
        close_webex_menu(application, picker);
        return Ok(Outcome::Switched);
    }
    set_attribute(&row, "AXSelected", CFBoolean::new(true).as_ref());
    // Space goes only to Webex at the front, with the list open on exactly this camera.
    let on_target = bool_attribute(&row, "AXSelected")? == Some(true);
    if !on_target || frontmost_pid() != Some(pid) || webex_camera_menu(application)?.is_none() {
        close_webex_menu(application, picker);
        return Ok(Outcome::Unconfirmed);
    }
    if cancelled() {
        close_webex_menu(application, picker);
        return Ok(Outcome::Cancelled);
    }
    if !post_key_chord_to_pid(pid, SPACE_KEY_CODE, &[]) {
        close_webex_menu(application, picker);
        return Ok(Outcome::Unconfirmed);
    }
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    if wait_for(step, || {
        webex_camera_menu(application)
            .ok()
            .flatten()
            .is_none()
            .then_some(())
    })
    .is_none()
    {
        close_webex_menu(application, picker);
        return Ok(Outcome::Unconfirmed);
    }
    if press(picker).is_err() {
        return Ok(Outcome::Unconfirmed);
    }
    let step = deadline.min(Instant::now() + STEP_TIMEOUT);
    let confirmed = wait_for(step, || webex_camera_menu(application).ok().flatten())
        .and_then(|menu| webex_camera_row(&menu, camera).ok().flatten())
        .is_some_and(|row| {
            labels_below(&row, ENTRY_LABEL_DEPTH)
                .is_ok_and(|labels| webex_entry_checked(camera, &labels))
        });
    close_webex_menu(application, picker);
    Ok(if confirmed {
        Outcome::Switched
    } else {
        Outcome::Unconfirmed
    })
}

/// Webex's open camera list, among its windows or as its focused window.
fn webex_camera_menu(application: &AXUIElement) -> Result<Option<CFRetained<AXUIElement>>> {
    let mut candidates = windows(application)?;
    if let Some(focused) = element_attribute(application, "AXFocusedWindow")? {
        candidates.push(focused);
    }
    for window in candidates {
        let identifier = string_attribute(&window, "AXIdentifier")?.unwrap_or_default();
        let description = string_attribute(&window, "AXDescription")?.unwrap_or_default();
        if is_webex_camera_menu(&identifier, &description) {
            return Ok(Some(window));
        }
    }
    Ok(None)
}

/// The row of the open Webex list that is the camera named `camera`.
fn webex_camera_row(menu: &AXUIElement, camera: &str) -> Result<Option<CFRetained<AXUIElement>>> {
    let Some(table) = first_descendant(menu, |role| role == "AXTable")? else {
        return Ok(None);
    };
    for row in element_list(&table, "AXRows")? {
        if names_camera(camera, &labels_below(&row, ENTRY_LABEL_DEPTH)?) {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

/// Closes Webex's camera list with the button that opened it, which toggles it, if it is open.
fn close_webex_menu(application: &AXUIElement, picker: &AXUIElement) {
    if webex_camera_menu(application).ok().flatten().is_some() {
        let _ = press(picker);
        thread::sleep(POLL_INTERVAL);
    }
}

/// The call and its picker, bringing the app to the front first when it shows its call only there.
fn find_call_at_front(
    app: NativeMeetingApp,
    pid: i32,
    application: &AXUIElement,
    deadline: Instant,
) -> Result<Option<Call>> {
    let found = find_call(app, application)?;
    let complete = found.as_ref().is_some_and(|call| call.picker.is_some());
    if complete || !app.shows_call_only_in_front() || !bring_to_front(pid, application, None) {
        return Ok(found);
    }
    let shown = wait_for(deadline.min(Instant::now() + STEP_TIMEOUT), || {
        find_call(app, application)
            .ok()
            .flatten()
            .filter(|call| call.picker.is_some())
    });
    Ok(shown.or(found))
}

/// The window holding a call, and the camera picker inside it.
fn find_call(app: NativeMeetingApp, application: &AXUIElement) -> Result<Option<Call>> {
    let mut found = None;
    for window in app_windows(application)? {
        let identifier = string_attribute(&window, "AXIdentifier")?.unwrap_or_default();
        let mut call = is_call_window(app, &identifier, &dom_classes(&window)?);
        let mut picker = None;
        let mut stack = vec![(window.clone(), 0_usize)];
        let mut visited = 0_usize;
        while let Some((element, depth)) = stack.pop() {
            visited += 1;
            if visited > MAX_SCAN_NODES {
                break;
            }
            let role = role_attribute(&element)?;
            let labels = labels_of(&element)?;
            if app == NativeMeetingApp::Teams && is_teams_call_marker(&role, &labels) {
                call = true;
            }
            if app == NativeMeetingApp::Webex
                && is_webex_call_marker(
                    &role,
                    &string_attribute(&element, "AXIdentifier")?.unwrap_or_default(),
                )
            {
                call = true;
            }
            if picker.is_none() && is_camera_picker(app, &role, &labels) {
                picker = Some(element.clone());
            }
            if call && picker.is_some() {
                break;
            }
            if depth < MAX_SCAN_DEPTH {
                for child in children(&element)?.into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
        }
        // Teams floats a compact view of the call, with its controls but no camera picker, while
        // it isn't at the front: a call window that holds the picker wins over it.
        if call && picker.is_some() {
            return Ok(Some(Call { window, picker }));
        }
        if call && found.is_none() {
            found = Some(Call { window, picker });
        }
    }
    Ok(found)
}

/// The entry naming `camera` in an open Teams flyout, in the call's window or a window of its own.
fn camera_entry(
    application: &AXUIElement,
    window: &AXUIElement,
    camera: &str,
) -> Result<Option<CFRetained<AXUIElement>>> {
    let mut roots = vec![retain_element(window)];
    for other in windows(application)? {
        if *other != *window {
            roots.push(other);
        }
    }
    for root in roots {
        let mut stack = vec![(root, 0_usize)];
        let mut visited = 0_usize;
        while let Some((element, depth)) = stack.pop() {
            visited += 1;
            if visited > MAX_SCAN_NODES {
                break;
            }
            let role = role_attribute(&element)?;
            if is_camera_entry(&role, &dom_classes(&element)?)
                && names_camera(camera, &labels_below(&element, 1)?)
            {
                return Ok(Some(element));
            }
            if depth < MAX_SCAN_DEPTH {
                for child in children(&element)? {
                    stack.push((child, depth + 1));
                }
            }
        }
    }
    Ok(None)
}

/// Whether the call's window shows a camera list: a menu, or Teams' flyout of camera list items.
fn camera_list_open(window: &AXUIElement) -> Result<bool> {
    let mut stack = vec![(retain_element(window), 0_usize)];
    let mut visited = 0_usize;
    while let Some((element, depth)) = stack.pop() {
        visited += 1;
        if visited > MAX_SCAN_NODES {
            break;
        }
        let role = role_attribute(&element)?;
        if matches!(role.as_str(), "AXMenu" | "AXMenuItem")
            || (role == "AXStaticText" && is_camera_entry(&role, &dom_classes(&element)?))
        {
            return Ok(true);
        }
        if depth < MAX_SCAN_DEPTH {
            for child in children(&element)? {
                stack.push((child, depth + 1));
            }
        }
    }
    Ok(false)
}

/// A window that wasn't open before the picker was pressed and holds a table: Zoom's menu.
fn new_window_with_table(
    application: &AXUIElement,
    before: &[CFRetained<AXUIElement>],
) -> Option<CFRetained<AXUIElement>> {
    let now = windows(application).ok()?;
    now.into_iter().find(|window| {
        !before.iter().any(|known| **known == **window)
            && first_descendant(window, |role| role == "AXTable")
                .ok()
                .flatten()
                .is_some()
    })
}

fn is_open(application: &AXUIElement, window: &AXUIElement) -> Result<bool> {
    Ok(windows(application)?.iter().any(|open| **open == *window))
}

fn first_descendant(
    root: &AXUIElement,
    matches: impl Fn(&str) -> bool,
) -> Result<Option<CFRetained<AXUIElement>>> {
    let mut queue = std::collections::VecDeque::from([(retain_element(root), 0_usize)]);
    let mut visited = 0_usize;
    while let Some((element, depth)) = queue.pop_front() {
        visited += 1;
        if visited > MAX_SCAN_NODES {
            break;
        }
        if matches(&role_attribute(&element)?) {
            return Ok(Some(element));
        }
        if depth < 6 {
            for child in children(&element)? {
                queue.push_back((child, depth + 1));
            }
        }
    }
    Ok(None)
}

fn labels_of(element: &AXUIElement) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    for attribute in [
        "AXTitle",
        "AXDescription",
        "AXHelp",
        "AXIdentifier",
        "AXDOMIdentifier",
    ] {
        if let Some(value) = string_attribute(element, attribute)? {
            labels.push(value);
        }
    }
    labels.extend(dom_classes(element)?);
    Ok(labels)
}

/// The labels of an element and of what sits up to `depth` levels below it, where list rows and
/// menu items keep their names.
fn labels_below(element: &AXUIElement, depth: usize) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    let mut stack = vec![(retain_element(element), 0_usize)];
    while let Some((current, level)) = stack.pop() {
        labels.extend(labels_of(&current)?);
        if let Some(value) = string_attribute(&current, "AXValue")? {
            labels.push(value);
        }
        if level < depth {
            for child in children(&current)? {
                stack.push((child, level + 1));
            }
        }
    }
    Ok(labels)
}

fn close_with_escape(pid: i32) {
    if frontmost_pid() == Some(pid) {
        post_key_chord_to_pid(pid, ESCAPE_KEY_CODE, &[]);
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for<T>(deadline: Instant, mut find: impl FnMut() -> Option<T>) -> Option<T> {
    loop {
        if let Some(found) = find() {
            return Some(found);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(POLL_INTERVAL);
    }
}
