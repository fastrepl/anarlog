use std::{collections::HashSet, future::Future, time::Duration};

use atspi::{
    AccessibilityConnection, Interface, ObjectRef, Role, State, StateSet,
    proxy::{
        accessible::{AccessibleProxy, ObjectRefExt},
        editable_text::EditableTextProxy,
        text::TextProxy,
    },
};
use desktop_runtime::{Result, ServiceError};

use super::{Insertion, NativeInsertion, validate};

const FOCUS_ERROR: &str =
    "Focus an editable text field in an application with accessibility enabled.";
const FIELD_CHANGED: &str =
    "The focused field changed. Copy your last dictation from Settings > Dictation.";

impl Insertion for NativeInsertion {
    fn capture(&self) -> Result<String> {
        run(async {
            let connection = AccessibilityConnection::new().await.map_err(failed)?;
            let target = focused_field(&connection).await?;
            serde_json::to_string(&target).map_err(failed)
        })
    }

    fn insert(&self, target: &str, text: &str) -> Result<()> {
        validate(text)?;
        let target = serde_json::from_str(target).map_err(failed)?;
        run(insert(target, text))
    }
}

fn run<T>(operation: impl Future<Output = Result<T>>) -> Result<T> {
    // Both calls are made on the dictation runtime's blocking workers.
    let runtime = tokio::runtime::Handle::try_current().map_err(failed)?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), operation)
            .await
            .map_err(|_| failed("Communicating with the focused field timed out."))?
    })
}

fn failed(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

async fn focused_field(connection: &AccessibilityConnection) -> Result<ObjectRef> {
    let root = AccessibleProxy::builder(connection.connection())
        .destination("org.a11y.atspi.Registry")
        .map_err(failed)?
        .path("/org/a11y/atspi/accessible/root")
        .map_err(failed)?
        .build()
        .await
        .map_err(failed)?;
    let mut pending = root.get_children().await.map_err(failed)?;
    let mut visited = HashSet::new();
    while let Some(object) = pending.pop() {
        if !visited.insert(object.clone()) {
            continue;
        }
        let Ok(proxy) = object.as_accessible_proxy(connection.connection()).await else {
            continue;
        };
        let Ok(states) = proxy.get_state().await else {
            continue;
        };
        if states.contains(State::Focused) && editable(&proxy, states).await? {
            return Ok(object);
        }
        if !states.contains(State::Active)
            && matches!(
                proxy.get_role().await,
                Ok(Role::Frame | Role::Window | Role::Dialog)
            )
        {
            continue;
        }
        if let Ok(children) = proxy.get_children().await {
            pending.extend(children);
        }
    }
    Err(failed(FOCUS_ERROR))
}

async fn editable(proxy: &AccessibleProxy<'_>, states: StateSet) -> Result<bool> {
    if !states.contains(State::Editable)
        || proxy.get_role().await.map_err(failed)? == Role::PasswordText
    {
        return Ok(false);
    }
    let interfaces = proxy.get_interfaces().await.map_err(failed)?;
    Ok(interfaces.contains(Interface::EditableText) && interfaces.contains(Interface::Text))
}

async fn insert(target: ObjectRef, text: &str) -> Result<()> {
    let connection = AccessibilityConnection::new().await.map_err(failed)?;
    if focused_field(&connection)
        .await
        .map_err(|_| failed(FIELD_CHANGED))?
        != target
    {
        return Err(failed(FIELD_CHANGED));
    }
    let accessible = target
        .as_accessible_proxy(connection.connection())
        .await
        .map_err(failed)?;
    let editable_text = EditableTextProxy::builder(connection.connection())
        .destination(target.name.clone())
        .map_err(failed)?
        .path(target.path.clone())
        .map_err(failed)?
        .build()
        .await
        .map_err(failed)?;
    let text_proxy = TextProxy::builder(connection.connection())
        .destination(target.name.clone())
        .map_err(failed)?
        .path(target.path.clone())
        .map_err(failed)?
        .build()
        .await
        .map_err(failed)?;
    // The AT-SPI 0.25 proxy generates GetNselections, but the bus spells it GetNSelections.
    let selections: i32 = text_proxy
        .inner()
        .call("GetNSelections", &())
        .await
        .map_err(failed)?;
    if selections > 0 {
        return Err(failed(
            "Clear the text selection before dictating, or copy your last dictation to replace it.",
        ));
    }
    let caret = text_proxy.caret_offset().await.map_err(failed)?;
    let states = accessible.get_state().await.map_err(failed)?;
    if caret < 0 || !states.contains(State::Focused) || !editable(&accessible, states).await? {
        return Err(failed(FIELD_CHANGED));
    }
    // AT-SPI takes the inserted string length in bytes and text offsets in characters.
    if !editable_text
        .insert_text(caret, text, text.len() as i32)
        .await
        .map_err(failed)?
    {
        return Err(failed(
            "This app could not insert text. Copy your last dictation from Settings > Dictation.",
        ));
    }
    if let Err(error) = text_proxy
        .set_caret_offset(caret.saturating_add(text.chars().count() as i32))
        .await
    {
        tracing::debug!(%error, "could not move caret after dictation");
    }
    Ok(())
}
