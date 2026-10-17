use crate::{Args, Result, cli::MeetingCommand, db, output};
use anlg_agent_access::{ImportRecordingInput, get_recording_import, import_recording};

pub async fn run(args: &Args, command: MeetingCommand, json: bool) -> Result<u8> {
    let operation = if matches!(&command, MeetingCommand::ImportStatus { .. }) {
        "meetings.import_status"
    } else {
        "meetings.import"
    };
    let (mut job, wait, timeout) = match command {
        MeetingCommand::Import {
            file,
            title,
            folder,
            request_key,
            wait,
            timeout,
        } => {
            let db = db::open_write(args).await?;
            let job = import_recording(
                db.pool(),
                ImportRecordingInput {
                    file_path: file.to_string_lossy().into_owned(),
                    title,
                    folder_path: folder,
                    request_key,
                },
            )
            .await?;
            (job, wait, timeout)
        }
        MeetingCommand::ImportStatus { id } => {
            let db = db::open(args).await?;
            (get_recording_import(db.pool(), &id).await?, false, 0)
        }
        _ => unreachable!("recording import command"),
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout);
    let db = db::open(args).await?;
    while wait && !matches!(job.processing_status.as_str(), "done" | "failed") {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        job = get_recording_import(db.pool(), &job.job_id).await?;
    }
    if json {
        output::emit(&output::json(operation, &job, None)?);
    } else {
        output::emit(&format!(
            "Meeting: {}\nJob: {}\nStatus: {}\nSummary: {}{}",
            job.meeting_id,
            job.job_id,
            job.processing_status,
            job.summary_status,
            job.error
                .as_ref()
                .map(|e| format!("\nError: {e}"))
                .unwrap_or_default()
        ));
    }
    Ok(if job.processing_status == "failed" {
        1
    } else if wait && job.processing_status != "done" {
        2
    } else {
        0
    })
}
