use atspi::proxy::{
    accessible::{AccessibleProxy, ObjectRefExt},
    editable_text::EditableTextProxy,
    text::TextProxy,
};
use atspi::{AccessibilityConnection, ObjectRef, Role, State};

const FOCUS_ERROR: &str = "Focus an editable text field in an application with accessibility enabled. Password fields are excluded.";

pub async fn capture() -> Result<String, String> {
    tokio::time::timeout(std::time::Duration::from_secs(3), capture_inner())
        .await
        .map_err(|_| {
            "Finding the focused field timed out. Enable accessibility in the target app."
                .to_string()
        })?
}

async fn capture_inner() -> Result<String, String> {
    let connection = AccessibilityConnection::new()
        .await
        .map_err(|e| e.to_string())?;
    let root = AccessibleProxy::builder(connection.connection())
        .destination("org.a11y.atspi.Registry")
        .map_err(|e| e.to_string())?
        .path("/org/a11y/atspi/accessible/root")
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let mut pending = root.get_children().await.map_err(|e| e.to_string())?;
    let mut visited = std::collections::HashSet::new();
    while let Some(object) = pending.pop() {
        if !visited.insert((object.name.to_string(), object.path.to_string())) {
            continue;
        }
        let Ok(proxy) = object
            .clone()
            .into_accessible_proxy(connection.connection())
            .await
        else {
            continue;
        };
        let Ok(states) = proxy.get_state().await else {
            continue;
        };
        if states.contains(State::Focused) {
            if !states.contains(State::Editable)
                || proxy.get_role().await.ok() == Some(Role::PasswordText)
            {
                return Err(FOCUS_ERROR.into());
            }
            return serde_json::to_string(&object).map_err(|e| e.to_string());
        }
        let role = proxy.get_role().await.ok();
        if matches!(role, Some(Role::Frame | Role::Window | Role::Dialog))
            && !states.contains(State::Active)
        {
            continue;
        }
        if let Ok(children) = proxy.get_children().await {
            pending.extend(children);
        }
    }
    Err(FOCUS_ERROR.into())
}

// The captured field is read and written through AT-SPI Text/EditableText. There is no
// clipboard paste on Linux: a field that refuses EditableText keeps its text.

const MAX_FIELD_CHARS: i32 = 200_000;

type Fallible<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const SETTLE: std::time::Duration = std::time::Duration::from_millis(40);

pub struct Field {
    object: ObjectRef,
    target: String,
    runtime: tokio::runtime::Handle,
}

/// Called on a blocking thread of the async runtime, which AT-SPI's calls run on.
pub fn field(target: &str) -> Option<Field> {
    Some(Field {
        object: serde_json::from_str(target).ok()?,
        target: target.to_owned(),
        runtime: tokio::runtime::Handle::try_current().ok()?,
    })
}

struct Parts<'a> {
    accessible: AccessibleProxy<'a>,
    text: TextProxy<'a>,
    editable: EditableTextProxy<'a>,
}

async fn parts<'a>(
    connection: &'a AccessibilityConnection,
    object: &ObjectRef,
) -> Fallible<Parts<'a>> {
    let accessible = object
        .clone()
        .into_accessible_proxy(connection.connection())
        .await?;
    let text = TextProxy::builder(connection.connection())
        .destination(object.name.clone())?
        .path(object.path.clone())?
        .build()
        .await?;
    let editable = EditableTextProxy::builder(connection.connection())
        .destination(object.name.clone())?
        .path(object.path.clone())?
        .build()
        .await?;
    Ok(Parts {
        accessible,
        text,
        editable,
    })
}

/// The field's whole value and its selection, in characters as AT-SPI counts them.
async fn contents(text: &TextProxy<'_>) -> Fallible<Option<(Vec<char>, i32, i32)>> {
    let count = text.character_count().await?;
    if !(0..=MAX_FIELD_CHARS).contains(&count) {
        return Ok(None);
    }
    let value: Vec<char> = text.get_text(0, count).await?.chars().collect();
    let (start, end) = if text.get_nselections().await? > 0 {
        let (a, b) = text.get_selection(0).await?;
        (a.min(b), a.max(b))
    } else {
        let caret = text.caret_offset().await?;
        (caret, caret)
    };
    let len = value.len() as i32;
    Ok(Some((value, start.clamp(0, len), end.clamp(0, len))))
}

fn utf16_offset(value: &[char], chars: i32) -> usize {
    value[..chars as usize].iter().map(|c| c.len_utf16()).sum()
}

fn char_offset(value: &[char], units: usize) -> Option<i32> {
    let mut seen = 0;
    for (index, c) in value.iter().enumerate() {
        if seen == units {
            return Some(index as i32);
        }
        seen += c.len_utf16();
    }
    (seen == units).then_some(value.len() as i32)
}

impl Field {
    fn run<T>(&self, work: impl AsyncFnOnce(&Parts<'_>) -> Fallible<T>) -> Option<T> {
        self.runtime.block_on(async {
            let connection = AccessibilityConnection::new().await.ok()?;
            let parts = parts(&connection, &self.object).await.ok()?;
            tokio::time::timeout(std::time::Duration::from_secs(3), work(&parts))
                .await
                .ok()?
                .ok()
        })
    }
}

impl super::edit::FieldPort for Field {
    fn focused(&self) -> bool {
        self.runtime
            .block_on(capture())
            .is_ok_and(|focused| focused == self.target)
    }

    fn read(&self) -> Option<super::edit::FieldRead> {
        let (value, start, end) =
            self.run(async |parts: &Parts<'_>| contents(&parts.text).await)??;
        let units: Vec<u16> = value.iter().collect::<String>().encode_utf16().collect();
        let (start, end) = (utf16_offset(&value, start), utf16_offset(&value, end));
        let from = start.saturating_sub(super::edit::WINDOW_UTF16);
        let to = (end + super::edit::WINDOW_UTF16).min(units.len());
        // Never split a surrogate pair at the window's edges.
        let from = if from > 0 && (0xDC00..=0xDFFF).contains(&units[from]) {
            from + 1
        } else {
            from
        };
        let to = if to < units.len() && (0xD800..=0xDBFF).contains(&units[to - 1]) {
            to - 1
        } else {
            to
        };
        Some(super::edit::FieldRead {
            length: units.len(),
            selection: super::edit::Span {
                start,
                len: end - start,
            },
            before: String::from_utf16_lossy(&units[from.min(start)..start]),
            selected: String::from_utf16_lossy(&units[start..end]),
            after: String::from_utf16_lossy(&units[end..to.max(end)]),
        })
    }

    fn select(&self, span: super::edit::Span) -> bool {
        self.run(async |parts: &Parts<'_>| {
            let Some((value, _, _)) = contents(&parts.text).await? else {
                return Ok(false);
            };
            let (Some(start), Some(end)) = (
                char_offset(&value, span.start),
                char_offset(&value, span.start + span.len),
            ) else {
                return Ok(false);
            };
            let selections = parts.text.get_nselections().await?;
            if start == end {
                for index in (0..selections).rev() {
                    parts.text.remove_selection(index).await?;
                }
                return Ok(parts.text.set_caret_offset(start).await?);
            }
            Ok(if selections > 0 {
                parts.text.set_selection(0, start, end).await?
            } else {
                parts.text.add_selection(start, end).await?
            })
        })
        .unwrap_or(false)
    }

    fn write(&self, text: &str) -> super::edit::Write {
        let written = self.run(async |parts: &Parts<'_>| {
            // Plain insertion can be allowed while privacy rules forbid reading the value.
            let count = parts.text.character_count().await?;
            if !(0..=MAX_FIELD_CHARS).contains(&count) {
                return Ok(None);
            }
            let (start, end) = if parts.text.get_nselections().await? > 0 {
                let (a, b) = parts.text.get_selection(0).await?;
                (a.min(b), a.max(b))
            } else {
                let caret = parts.text.caret_offset().await?;
                (caret, caret)
            };
            if start < 0 || end > count {
                return Ok(None);
            }
            // Replacing a selection is two edits; the read-back tells a half-done one apart.
            let deleted = start != end && parts.editable.delete_text(start, end).await?;
            if start != end && !deleted {
                return Ok(None);
            }
            let inserted = parts
                .editable
                .insert_text(start, text, text.len() as i32)
                .await?;
            if inserted {
                let _ = parts
                    .text
                    .set_caret_offset(start + text.chars().count() as i32)
                    .await;
            }
            Ok(Some(inserted || deleted))
        });
        match written.flatten() {
            Some(true) => super::edit::Write::Accepted,
            _ => super::edit::Write::Rejected,
        }
    }

    fn paste(&self, _text: &str) -> super::edit::Paste {
        super::edit::Paste::NotSent
    }

    fn settle(&self) {
        std::thread::sleep(SETTLE);
    }
}

pub fn facts(target: &str) -> Option<dictation_core::context::WindowFacts> {
    let object: ObjectRef = serde_json::from_str(target).ok()?;
    let runtime = tokio::runtime::Handle::try_current().ok()?;
    runtime.block_on(async {
        let atspi = AccessibilityConnection::new().await.ok()?;
        let parts = parts(&atspi, &object).await.ok()?;
        let secure = parts.accessible.get_role().await.ok()? == Role::PasswordText;
        let connection = atspi.connection();
        let application = parts.accessible.get_application().await.ok()?;
        let app_name = application
            .into_accessible_proxy(connection)
            .await
            .ok()?
            .name()
            .await
            .ok()
            .filter(|name| !name.trim().is_empty());
        // AT-SPI names the app ("Firefox", "Google Chrome"); its lowercase, dashed form stands
        // in for a bundle id the way desktop app ids do.
        let bundle_id = app_name
            .as_deref()
            .map(|name| name.trim().to_lowercase().replace(' ', "-"));
        let mut title = None;
        let mut node = parts.accessible.clone();
        for _ in 0..30 {
            let Ok(parent) = node.parent().await else {
                break;
            };
            let Ok(parent) = parent.into_accessible_proxy(connection).await else {
                break;
            };
            if matches!(
                parent.get_role().await,
                Ok(Role::Frame | Role::Window | Role::Dialog)
            ) {
                title = parent.name().await.ok().filter(|name| !name.is_empty());
                break;
            }
            node = parent;
        }
        let browser = bundle_id
            .as_deref()
            .is_some_and(dictation_core::context::is_browser);
        Some(dictation_core::context::WindowFacts {
            bundle_id,
            app_name,
            title,
            secure_field: Some(secure),
            private_window: (!browser).then_some(false),
            browser_host: None,
        })
    })
}

// Window text is not gathered over AT-SPI yet; context carries the field's own text only.
pub fn window_text(_target: &str, _max_chars: usize) -> Option<String> {
    None
}

pub fn release(_target: &str) {}

// No output volume control is wired up on Linux.
pub fn system_volume() -> Option<Box<dyn super::volume::SystemVolume>> {
    None
}
