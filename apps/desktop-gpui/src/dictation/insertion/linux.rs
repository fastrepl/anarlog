use std::{collections::HashSet, future::Future, time::Duration};

use atspi::{
    AccessibilityConnection, Interface, ObjectRef, Role, State, StateSet,
    proxy::{
        accessible::{AccessibleProxy, ObjectRefExt},
        application::ApplicationProxy,
        editable_text::EditableTextProxy,
        text::TextProxy,
    },
    zbus::proxy::CacheProperties,
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
            check_toolkit(&target, &connection).await?;
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
        // A stalled background application should not consume the whole search deadline.
        match tokio::time::timeout(Duration::from_millis(250), inspect(&object, connection)).await {
            Ok(Ok(AccessibleNode::FocusedField)) => return Ok(object),
            Ok(Ok(AccessibleNode::Children(children))) => pending.extend(children),
            _ => {}
        }
    }
    Err(failed(FOCUS_ERROR))
}

enum AccessibleNode {
    FocusedField,
    Children(Vec<ObjectRef>),
}

async fn inspect(
    object: &ObjectRef,
    connection: &AccessibilityConnection,
) -> Result<AccessibleNode> {
    let proxy = object
        .as_accessible_proxy(connection.connection())
        .await
        .map_err(failed)?;
    let states = proxy.get_state().await.map_err(failed)?;
    if states.contains(State::Focused) && editable(&proxy, states).await? {
        return Ok(AccessibleNode::FocusedField);
    }
    if !states.contains(State::Active)
        && matches!(
            proxy.get_role().await,
            Ok(Role::Frame | Role::Window | Role::Dialog)
        )
    {
        return Ok(AccessibleNode::Children(Vec::new()));
    }
    Ok(AccessibleNode::Children(
        proxy.get_children().await.map_err(failed)?,
    ))
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

async fn check_toolkit(target: &ObjectRef, connection: &AccessibilityConnection) -> Result<()> {
    let accessible = target
        .as_accessible_proxy(connection.connection())
        .await
        .map_err(failed)?;
    let app = accessible.get_application().await.map_err(failed)?;
    let toolkit = ApplicationProxy::builder(connection.connection())
        .destination(app.name)
        .map_err(failed)?
        .path(app.path)
        .map_err(failed)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(failed)?
        .toolkit_name()
        .await
        .map_err(failed)?;
    // Qt can resize the transcript and rewrite the entire field when inserting via AT-SPI.
    if toolkit.eq_ignore_ascii_case("Qt") {
        return Err(failed(
            "Dictation insertion is unavailable in Qt text fields.",
        ));
    }
    Ok(())
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
    check_toolkit(&target, &connection).await?;
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
        // Some AT-SPI bridges support individual properties but not GetAll.
        .cache_properties(CacheProperties::No)
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
    let original_length = text_proxy.character_count().await.map_err(failed)?;
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
    let inserted_length = text.chars().count() as i32;
    let updated_length = text_proxy.character_count().await.map_err(|_| {
        failed("Could not verify the inserted dictation. Review the field or copy the transcript from Settings > Dictation.")
    })?;
    if updated_length.checked_sub(original_length) != Some(inserted_length) {
        return Err(failed(
            "The app did not insert the full dictation. Review the field or copy the transcript from Settings > Dictation.",
        ));
    }
    if let Err(error) = text_proxy
        .set_caret_offset(caret.saturating_add(inserted_length))
        .await
    {
        tracing::debug!(%error, "could not move caret after dictation");
    }
    Ok(())
}
