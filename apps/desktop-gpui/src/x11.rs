//! X11 window placement, hiding and attention. gpui 0.2.2 creates the X
//! window at the requested origin but never asks the window manager to
//! honour it (no `PPosition` hint), so the manager places the window itself
//! and a restored `.window-state.json` position is lost. Tauri's GTK window
//! moves to its saved origin; the shell does the same with a
//! `ConfigureWindow` on the mapped window, found through the `_NET_WM_PID`
//! gpui stamps on it. gpui also has no `hide`, so the tray's hide/show is
//! the ICCCM withdraw and a fresh map done here.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::properties::WmHints;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt, EventMask, MapState, UNMAP_NOTIFY_EVENT,
    UnmapNotifyEvent, Window,
};

/// The window withdrawn by [`withdraw`], mapped again by [`map_withdrawn`].
static WITHDRAWN: AtomicU32 = AtomicU32::new(0);

/// `window.hide()` as GTK does it: the ICCCM withdraw (unmap plus a
/// synthetic `UnmapNotify` to the root) of this process's mapped `width` ×
/// `height` window, so the window manager drops it from the taskbar and
/// pager instead of iconifying it. `false` when there is no X server or no
/// such window.
pub fn withdraw(width: u32, height: u32) -> bool {
    let result: anyhow::Result<bool> = (|| {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let pid_atom = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        let pid = std::process::id();
        let Some(window) = find_window(&conn, root, pid_atom, pid, width, height)? else {
            return Ok(false);
        };
        conn.unmap_window(window)?;
        let notify = UnmapNotifyEvent {
            response_type: UNMAP_NOTIFY_EVENT,
            sequence: 0,
            event: root,
            window,
            from_configure: false,
        };
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            notify,
        )?;
        conn.flush()?;
        WITHDRAWN.store(window, Ordering::SeqCst);
        tracing::debug!(window, "withdrew the main window");
        Ok(true)
    })();
    match result {
        Ok(withdrawn) => withdrawn,
        Err(error) => {
            tracing::debug!(%error, "x11 withdraw unavailable");
            false
        }
    }
}

/// `window.show()`: maps the withdrawn window again (a no-op once it is
/// mapped); the window manager places it like GTK's shown window.
pub fn map_withdrawn() {
    let window = WITHDRAWN.load(Ordering::SeqCst);
    if window == 0 {
        return;
    }
    let result: anyhow::Result<()> = (|| {
        let (conn, _) = x11rb::connect(None)?;
        conn.map_window(window)?;
        // The round trip keeps the connection open until the server has
        // handed the MapRequest to the window manager; a request still in
        // flight when the connection closes is dropped.
        conn.get_window_attributes(window)?.reply()?;
        Ok(())
    })();
    match result {
        Ok(()) => tracing::debug!(window, "mapped the withdrawn main window"),
        Err(error) => tracing::debug!(%error, "x11 map unavailable"),
    }
}

/// Moves this process's mapped top-level window of `width` × `height` to
/// (`x`, `y`) once the window manager has shown it. Runs off the UI thread
/// and gives up quietly when there is no X server or no such window.
pub fn move_window_when_mapped(width: u32, height: u32, x: i32, y: i32) {
    std::thread::Builder::new()
        .name("x11-window-move".into())
        .spawn(move || {
            for _ in 0..40 {
                match try_move(width, height, x, y) {
                    Ok(true) => {
                        tracing::debug!(x, y, "moved the main window to its saved origin");
                        return;
                    }
                    Ok(false) => std::thread::sleep(Duration::from_millis(50)),
                    Err(error) => {
                        tracing::debug!(%error, "x11 window move unavailable");
                        return;
                    }
                }
            }
            tracing::debug!("the main window did not map in time to be moved");
        })
        .ok();
}

fn try_move(width: u32, height: u32, x: i32, y: i32) -> anyhow::Result<bool> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let pid_atom = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
    let pid = std::process::id();
    let Some(window) = find_window(&conn, root, pid_atom, pid, width, height)? else {
        return Ok(false);
    };
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;
    Ok(true)
}

/// `requestUserAttention(Informational)` as GTK does it: the `WM_HINTS`
/// urgency flag on this process's mapped `width` × `height` window, which
/// the window manager shows as a demanding taskbar entry; `false` clears it
/// again once the window is active. Runs off the UI thread.
pub fn set_urgent(width: u32, height: u32, urgent: bool) {
    std::thread::Builder::new()
        .name("x11-attention".into())
        .spawn(move || {
            if let Err(error) = try_set_urgent(width, height, urgent) {
                tracing::debug!(%error, urgent, "x11 urgency hint unavailable");
            }
        })
        .ok();
}

fn try_set_urgent(width: u32, height: u32, urgent: bool) -> anyhow::Result<()> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let pid_atom = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
    let pid = std::process::id();
    let Some(window) = find_window(&conn, root, pid_atom, pid, width, height)? else {
        anyhow::bail!("main window not found");
    };
    let mut hints = WmHints::get(&conn, window)?.reply()?.unwrap_or_default();
    if hints.urgent == urgent {
        return Ok(());
    }
    hints.urgent = urgent;
    hints.set(&conn, window)?;
    conn.flush()?;
    Ok(())
}

fn find_window(
    conn: &impl Connection,
    window: Window,
    pid_atom: u32,
    pid: u32,
    width: u32,
    height: u32,
) -> anyhow::Result<Option<Window>> {
    let owner = conn
        .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)?
        .reply()?
        .value32()
        .and_then(|mut values| values.next());
    if owner == Some(pid) {
        let geometry = conn.get_geometry(window)?.reply()?;
        let attributes = conn.get_window_attributes(window)?.reply()?;
        let fits = u32::from(geometry.width).abs_diff(width) <= 2
            && u32::from(geometry.height).abs_diff(height) <= 2;
        if fits && attributes.map_state == MapState::VIEWABLE {
            return Ok(Some(window));
        }
    }
    for child in conn.query_tree(window)?.reply()?.children {
        if let Some(found) = find_window(conn, child, pid_atom, pid, width, height)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// `clipboardData.getData("text/html")`: the `text/html` target of the
/// CLIPBOARD selection, which gpui's clipboard (text and images) does not
/// read. `None` when the owner offers no HTML or does not answer in time.
pub fn clipboard_html() -> Option<String> {
    use std::sync::{Mutex, OnceLock};
    use x11_clipboard::Clipboard;
    static CLIPBOARD: OnceLock<Option<Mutex<Clipboard>>> = OnceLock::new();
    let clipboard = CLIPBOARD
        .get_or_init(|| match Clipboard::new() {
            Ok(clipboard) => Some(Mutex::new(clipboard)),
            Err(error) => {
                tracing::debug!(%error, "x11 clipboard unavailable");
                None
            }
        })
        .as_ref()?;
    let clipboard = clipboard.lock().ok()?;
    let target = clipboard.getter.get_atom("text/html").ok()?;
    let bytes = clipboard
        .load(
            clipboard.getter.atoms.clipboard,
            target,
            clipboard.getter.atoms.property,
            Duration::from_millis(250),
        )
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    // Some owners hand out UTF-16 with a byte-order mark.
    let text = match bytes.as_slice() {
        [0xff, 0xfe, rest @ ..] => String::from_utf16_lossy(
            &rest
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>(),
        ),
        [0xfe, 0xff, rest @ ..] => String::from_utf16_lossy(
            &rest
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>(),
        ),
        _ => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let text = text.trim_end_matches('\0').to_string();
    (!text.trim().is_empty()).then_some(text)
}

/// `ClipboardEvent` on copy: the selection's text next to the HTML
/// ProseMirror's clipboard serializer builds, the way the web view offers
/// `text/plain` and `text/html` together. gpui's clipboard writes a string
/// alone, so a thread of this process owns the CLIPBOARD selection and
/// answers the targets until another owner (the next copy, gpui, or any
/// other app) takes it. `false` when no X server is available; the caller
/// falls back to gpui's clipboard.
pub fn write_clipboard(text: String, html: String) -> bool {
    use x11rb::CURRENT_TIME;
    use x11rb::connection::RequestConnection as _;
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
        SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, WindowClass,
    };
    use x11rb::wrapper::ConnectionExt as _;

    let Ok((conn, screen)) = x11rb::connect(None) else {
        return false;
    };
    let root = conn.setup().roots[screen].root;
    let window = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return false,
    };
    if conn
        .create_window(
            0,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .is_err()
    {
        return false;
    }
    let atom = |name: &str| -> Option<Atom> {
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()
            .map(|r| r.atom)
    };
    let (
        Some(clipboard),
        Some(targets),
        Some(timestamp),
        Some(utf8),
        Some(text_atom),
        Some(plain),
        Some(plain_utf8),
        Some(html_atom),
    ) = (
        atom("CLIPBOARD"),
        atom("TARGETS"),
        atom("TIMESTAMP"),
        atom("UTF8_STRING"),
        atom("TEXT"),
        atom("text/plain"),
        atom("text/plain;charset=utf-8"),
        atom("text/html"),
    )
    else {
        return false;
    };
    if conn
        .set_selection_owner(window, clipboard, CURRENT_TIME)
        .is_err()
        || conn.flush().is_err()
    {
        return false;
    }
    let owner = conn
        .get_selection_owner(clipboard)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.owner);
    if owner != Some(window) {
        return false;
    }
    // Properties beyond the request limit would need INCR; such a target is
    // refused and the requestor takes the text instead.
    let limit = conn.maximum_request_bytes().saturating_sub(64);
    let latin1: Vec<u8> = text
        .chars()
        .map(|c| {
            if (c as u32) < 256 {
                c as u32 as u8
            } else {
                b'?'
            }
        })
        .collect();
    std::thread::Builder::new()
        .name("x11-clipboard-owner".into())
        .spawn(move || {
            loop {
                let Ok(event) = conn.wait_for_event() else {
                    return;
                };
                match event {
                    Event::SelectionClear(clear) if clear.selection == clipboard => return,
                    Event::SelectionRequest(request) if request.selection == clipboard => {
                        let property = if request.property == Atom::from(AtomEnum::NONE) {
                            request.target
                        } else {
                            request.property
                        };
                        let served = if request.target == targets {
                            let list: Vec<Atom> = vec![
                                targets,
                                timestamp,
                                utf8,
                                text_atom,
                                plain,
                                plain_utf8,
                                html_atom,
                                Atom::from(AtomEnum::STRING),
                            ];
                            conn.change_property32(
                                PropMode::REPLACE,
                                request.requestor,
                                property,
                                AtomEnum::ATOM,
                                &list,
                            )
                            .is_ok()
                        } else if request.target == timestamp {
                            conn.change_property32(
                                PropMode::REPLACE,
                                request.requestor,
                                property,
                                AtomEnum::INTEGER,
                                &[CURRENT_TIME],
                            )
                            .is_ok()
                        } else if request.target == utf8
                            || request.target == text_atom
                            || request.target == plain
                            || request.target == plain_utf8
                        {
                            text.len() <= limit
                                && conn
                                    .change_property8(
                                        PropMode::REPLACE,
                                        request.requestor,
                                        property,
                                        if request.target == text_atom {
                                            utf8
                                        } else {
                                            request.target
                                        },
                                        text.as_bytes(),
                                    )
                                    .is_ok()
                        } else if request.target == Atom::from(AtomEnum::STRING) {
                            latin1.len() <= limit
                                && conn
                                    .change_property8(
                                        PropMode::REPLACE,
                                        request.requestor,
                                        property,
                                        AtomEnum::STRING,
                                        &latin1,
                                    )
                                    .is_ok()
                        } else if request.target == html_atom {
                            html.len() <= limit
                                && conn
                                    .change_property8(
                                        PropMode::REPLACE,
                                        request.requestor,
                                        property,
                                        html_atom,
                                        html.as_bytes(),
                                    )
                                    .is_ok()
                        } else {
                            false
                        };
                        let notify = SelectionNotifyEvent {
                            response_type: SELECTION_NOTIFY_EVENT,
                            sequence: 0,
                            time: request.time,
                            requestor: request.requestor,
                            selection: request.selection,
                            target: request.target,
                            property: if served {
                                property
                            } else {
                                Atom::from(AtomEnum::NONE)
                            },
                        };
                        let _ =
                            conn.send_event(false, request.requestor, EventMask::NO_EVENT, notify);
                        let _ = conn.flush();
                    }
                    _ => {}
                }
            }
        })
        .is_ok()
}
