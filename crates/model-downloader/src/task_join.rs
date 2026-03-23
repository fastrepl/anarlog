use std::time::Duration;

use tokio::task::JoinHandle;

pub(crate) async fn wait_for_task_exit(
    task: JoinHandle<()>,
    warn_after: Duration,
    context: &'static str,
) {
    let warn_after_sleep = tokio::time::sleep(warn_after);
    tokio::pin!(warn_after_sleep);
    tokio::pin!(task);

    let join_result = tokio::select! {
        result = &mut task => result,
        _ = &mut warn_after_sleep => {
            tracing::warn!(
                %context,
                hyprnote.timeout_s = warn_after.as_secs(),
                "model_download_task_join_slow"
            );
            task.await
        }
    };

    match join_result {
        Ok(()) => {}
        Err(e) => {
            tracing::warn!(%context, error = %e, "model_download_task_join_failed");
        }
    }
}
