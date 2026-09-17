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

pub async fn insert(target: String, text: String) -> Result<(), String> {
    if capture().await? != target {
        return Err(
            "The focused field changed. Copy your last dictation from Settings > Dictation.".into(),
        );
    }
    let object: ObjectRef = serde_json::from_str(&target).map_err(|e| e.to_string())?;
    let connection = AccessibilityConnection::new()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = object
        .clone()
        .into_accessible_proxy(connection.connection())
        .await
        .map_err(|e| e.to_string())?;
    let states = proxy.get_state().await.map_err(|e| e.to_string())?;
    if !states.contains(State::Focused)
        || !states.contains(State::Editable)
        || proxy.get_role().await.map_err(|e| e.to_string())? == Role::PasswordText
    {
        return Err(
            "The focused field changed. Copy your last dictation from Settings > Dictation.".into(),
        );
    }
    let editable = EditableTextProxy::builder(connection.connection())
        .destination(object.name.clone())
        .map_err(|e| e.to_string())?
        .path(object.path.clone())
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let text_proxy = TextProxy::builder(connection.connection())
        .destination(object.name)
        .map_err(|e| e.to_string())?
        .path(object.path)
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    // Inserting through EditableText works under both X11 and Wayland, without simulating keystrokes.
    if text_proxy
        .get_nselections()
        .await
        .map_err(|e| e.to_string())?
        > 0
    {
        return Err(
            "Clear the text selection before dictating, or copy your last dictation to replace it."
                .into(),
        );
    }
    let caret = text_proxy.caret_offset().await.map_err(|e| e.to_string())?;
    if !editable
        .insert_text(caret, &text, text.len() as i32)
        .await
        .map_err(|e| e.to_string())?
    {
        return Err(
            "This app could not insert text. Copy your last dictation from Settings > Dictation."
                .into(),
        );
    }
    let _ = text_proxy
        .set_caret_offset(caret + text.chars().count() as i32)
        .await;
    Ok(())
}
