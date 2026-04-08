mod display;
mod server;

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use owhisper_interface::batch_sse::{BatchSseMessage, EVENT_NAME};

struct Args {
    model: PathBuf,
    file: PathBuf,
    sse: bool,
    languages: Vec<String>,
}

impl Args {
    fn parse() -> Self {
        let mut args = pico_args::Arguments::from_env();

        let model: PathBuf = args.value_from_str("--model").unwrap_or_else(|_| {
            eprintln!("error: --model <PATH> is required");
            std::process::exit(1);
        });

        let file: PathBuf = args
            .opt_value_from_str("--file")
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
            .unwrap_or_else(|| PathBuf::from(hypr_data::english_1::AUDIO_PATH));

        let no_sse: Option<String> = args.opt_value_from_str("--sse").unwrap_or(None);
        let sse = !matches!(no_sse.as_deref(), Some("0" | "false"));

        let mut languages: Vec<String> = Vec::new();
        while let Ok(lang) = args.value_from_str::<_, String>("--language") {
            languages.push(lang);
        }
        if languages.is_empty() {
            languages.push("en".to_string());
        }

        Self {
            model,
            file,
            sse,
            languages,
        }
    }
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("m4a") => "audio/mp4",
        Some("webm") => "audio/webm",
        _ => "application/octet-stream",
    }
}

/// cargo run -p transcribe-cactus --example batch -- --sse --language en --language ko --model ~/Library/Application\ Support/hyprnote/models/cactus/parakeet-tdt-0.6b-v3-int4 --file /Users/yujonglee/dev/char/crates/data/src/english_10/audio.mp3
#[tokio::main]
async fn main() {
    let args = Args::parse();

    assert!(
        args.model.exists(),
        "model not found: {}",
        args.model.display()
    );
    assert!(
        args.file.exists(),
        "audio file not found: {}",
        args.file.display()
    );

    let audio_bytes = std::fs::read(&args.file).expect("failed to read audio file");
    let content_type = content_type_for_path(&args.file);

    display::print_header(
        &args.file.display().to_string(),
        audio_bytes.len(),
        content_type,
        &args.model.display().to_string(),
        args.sse,
        &args.languages,
    );

    let server = server::spawn(args.model).await;
    let t0 = std::time::Instant::now();
    let lang_query: String = args
        .languages
        .iter()
        .map(|l| format!("language={l}"))
        .collect::<Vec<_>>()
        .join("&");
    let url = format!("http://{}/v1/listen?{}", server.addr, lang_query);

    if args.sse {
        run_sse(&url, content_type, audio_bytes, t0).await;
    } else {
        run_sync(&url, content_type, audio_bytes, t0).await;
    }
}

async fn run_sync(url: &str, content_type: &str, body: Vec<u8>, t0: std::time::Instant) {
    let response = reqwest::Client::new()
        .post(url)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        display::print_error(&format!("HTTP {status}"), &text);
        return;
    }

    let result: owhisper_interface::batch::Response =
        response.json().await.expect("failed to parse response");
    display::print_result(&result, t0);
}

async fn run_sse(url: &str, content_type: &str, body: Vec<u8>, t0: std::time::Instant) {
    let response = reqwest::Client::new()
        .post(url)
        .header("content-type", content_type)
        .header("accept", "text/event-stream")
        .body(body)
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        display::print_error(&format!("HTTP {status}"), &text);
        return;
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut last_was_segment = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("stream error");
        buffer.extend_from_slice(&chunk);

        while let Some(end) = find_block_end(&buffer) {
            let block = String::from_utf8_lossy(&buffer[..end]).to_string();
            buffer.drain(..end + 2);

            if let Some(msg) = parse_sse_block(&block) {
                match msg {
                    BatchSseMessage::Progress { progress } => {
                        if !last_was_segment {
                            display::print_progress(&progress, t0);
                        }
                        last_was_segment = false;
                    }
                    BatchSseMessage::Segment { response } => {
                        display::print_segment(&response, t0);
                        last_was_segment = true;
                    }
                    BatchSseMessage::Result { response } => {
                        eprintln!();
                        display::print_result(&response, t0);
                        return;
                    }
                    BatchSseMessage::Error { error, detail } => {
                        eprintln!();
                        display::print_error(&error, &detail);
                        return;
                    }
                }
            }
        }
    }
}

fn find_block_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|w| w == b"\n\n")
}

fn parse_sse_block(block: &str) -> Option<BatchSseMessage> {
    let mut event_type = "";
    let mut data = String::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = rest.trim();
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim());
        }
    }

    if data.is_empty() || event_type != EVENT_NAME {
        return None;
    }

    serde_json::from_str(&data).ok()
}
