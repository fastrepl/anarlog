use std::sync::Arc;

use tauri::Manager;

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_target() -> Result<String, String> {
    crate::insertion::capture().await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn begin_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::engine::BeginDictation,
    updates: tauri::ipc::Channel<crate::engine::DictationUpdate>,
) -> Result<crate::engine::DictationTicket, crate::engine::DictationError> {
    let audio = app
        .state::<Arc<dyn anlg_audio::AudioProvider>>()
        .inner()
        .clone();
    app.state::<crate::engine::Engine>()
        .begin(request, audio, updates)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn attach_dictation_stream<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    stream: crate::preview::PreviewConfig,
) -> Result<(), crate::engine::DictationError> {
    app.state::<crate::engine::Engine>()
        .attach_stream(&operation_id, stream)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn finish_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
) -> Result<crate::engine::DictationFinish, crate::engine::DictationError> {
    app.state::<crate::engine::Engine>()
        .finish(&operation_id)
        .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn complete_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    batch_text: Option<String>,
) -> Result<String, crate::engine::DictationError> {
    app.state::<crate::engine::Engine>()
        .complete(&operation_id, batch_text)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn release_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    keep_for_recovery: bool,
) -> Result<Option<String>, crate::engine::DictationError> {
    app.state::<crate::engine::Engine>()
        .release(&operation_id, keep_for_recovery)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
) -> Result<(), crate::engine::DictationError> {
    app.state::<crate::engine::Engine>()
        .cancel(&operation_id)
        .await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn retire_dictations<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    reason: crate::engine::RetireReason,
) -> Result<(), crate::engine::DictationError> {
    crate::insertion::retire_targets(match &reason {
        crate::engine::RetireReason::Meeting => None,
        crate::engine::RetireReason::Account { account_id } => Some(account_id),
    });
    app.state::<crate::engine::Engine>().retire(reason).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_dictation_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: String,
) -> Result<Vec<crate::recovery::DictationRecoveryItem>, crate::engine::DictationError> {
    let engine = app.state::<crate::engine::Engine>();
    engine
        .recovery()?
        .list(&account_id, crate::engine::now_ms())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn prepare_dictation_recovery_audio<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: String,
    id: String,
    policy: crate::engine::DictationPolicy,
) -> Result<String, crate::engine::DictationError> {
    let engine = app.state::<crate::engine::Engine>();
    engine
        .recovery()?
        .prepare_audio(&account_id, &id, &policy, crate::engine::now_ms())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save_dictation_recovery_text<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: String,
    id: String,
    text: String,
) -> Result<(), crate::engine::DictationError> {
    let engine = app.state::<crate::engine::Engine>();
    engine
        .recovery()?
        .save_text(&account_id, &id, text, crate::engine::now_ms())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn claim_dictation_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: String,
    id: String,
) -> Result<String, crate::engine::DictationError> {
    let engine = app.state::<crate::engine::Engine>();
    engine
        .recovery()?
        .claim(&account_id, &id, crate::engine::now_ms())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn discard_dictation_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: String,
    id: String,
) -> Result<(), crate::engine::DictationError> {
    let engine = app.state::<crate::engine::Engine>();
    engine
        .recovery()?
        .discard(&account_id, &id, crate::engine::now_ms())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn fit_dictation_text(
    text: String,
    before: Option<String>,
    can_join: bool,
) -> dictation_core::CaretFit {
    dictation_core::fit_to_caret(&text, before.as_deref(), can_join)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn prepare_dictation_refinement(
    request: dictation_core::RefineRequest,
) -> dictation_core::RefinePlan {
    dictation_core::prepare_refinement(request)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_dictation_refinement(
    pending: dictation_core::PendingRefinement,
    reply: dictation_core::ModelReply,
) -> dictation_core::RefineOutcome {
    dictation_core::resolve_refinement(pending, reply)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn suggest_dictation_terms(
    request: dictation_core::SuggestRequest,
) -> Vec<String> {
    dictation_core::suggest_terms(&request)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn rank_dictation_terms(request: dictation_core::RankRequest) -> Vec<String> {
    dictation_core::rank_terms(&request)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn record_dictation_terms(request: dictation_core::RecordRequest) -> String {
    dictation_core::record_terms(request)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_dictation_context(
    target: String,
    request: crate::insertion::ContextRequest,
) -> Result<crate::insertion::TargetContext, String> {
    crate::insertion::read_context(target, request).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn insert_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: String,
    request: crate::insertion::InsertRequest,
) -> Result<crate::insertion::InsertOutcome, String> {
    let operation_id = request.operation_id.clone();
    crate::insertion::insert_dictation(target, request, move || {
        app.state::<crate::engine::Engine>()
            .deliverable(&operation_id)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_dictation_kept(
    target: String,
    request: crate::insertion::ContextRequest,
    edit_id: String,
) -> Result<Option<String>, String> {
    crate::insertion::read_kept(target, request, edit_id).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn release_dictation_target(target: String) -> Result<(), String> {
    crate::insertion::release_target(target).await
}
