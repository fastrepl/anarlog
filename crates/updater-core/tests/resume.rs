use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use updater_core::{ResolvedRelease, UpdateChecker, UpdateConfig, Version};

// Public test vector from minisign-verify, signing the four bytes "test".
const PUBKEY: &str = "untrusted comment: minisign public key
RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
const SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:test
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==";

fn checker(url: &str) -> (UpdateChecker, ResolvedRelease) {
    let mut config = UpdateConfig::new(
        vec![url.parse().unwrap()],
        Version::new(1, 0, 0),
        "darwin-aarch64",
        STANDARD.encode(PUBKEY),
    );
    config.no_proxy = true;
    config.timeout = Some(Duration::from_secs(5));
    let release = ResolvedRelease {
        version: Version::new(1, 0, 1),
        notes: None,
        pub_date: None,
        target: config.target.clone(),
        download_url: url.parse().unwrap(),
        signature: STANDARD.encode(SIGNATURE),
        raw_json: serde_json::Value::Null,
    };
    (UpdateChecker::new(config).unwrap(), release)
}

async fn request(listener: &tokio::net::TcpListener) -> (tokio::net::TcpStream, String) {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
    }
    (socket, String::from_utf8(bytes).unwrap().to_lowercase())
}

#[tokio::test]
async fn interrupted_download_resumes_saved_bytes_after_process_exit() {
    if let Ok(url) = std::env::var("ANARLOG_RESUME_TEST_URL") {
        let path = std::env::var("ANARLOG_RESUME_TEST_PATH").unwrap();
        let (checker, release) = checker(&url);
        checker
            .download_to_path(&release, std::path::Path::new(&path), |bytes, _| {
                if bytes > 0 {
                    println!("partial saved");
                }
            })
            .await
            .unwrap();
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("1.0.1.bin");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/update", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut first, headers) = request(&listener).await;
        assert!(!headers.contains("range:"));
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nte")
            .await
            .unwrap();

        let (mut second, headers) = request(&listener).await;
        assert!(headers.contains("range: bytes=2-\r\n"));
        second.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 2\r\nContent-Range: bytes 2-3/4\r\n\r\nst").await.unwrap();
    });
    let mut process = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "interrupted_download_resumes_saved_bytes_after_process_exit",
            "--nocapture",
        ])
        .env("ANARLOG_RESUME_TEST_URL", &url)
        .env("ANARLOG_RESUME_TEST_PATH", &destination)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = BufReader::new(process.stdout.take().unwrap()).lines();
    tokio::time::timeout(Duration::from_secs(5), async {
        while output
            .next_line()
            .await
            .unwrap()
            .expect("download process exited")
            != "partial saved"
        {}
    })
    .await
    .unwrap();
    process.kill().await.unwrap();
    process.wait().await.unwrap();
    assert!(!destination.exists());
    assert_eq!(
        std::fs::read(destination.with_extension("part")).unwrap(),
        b"te"
    );

    let (reopened, release) = checker(&url);
    let mut progress = Vec::new();
    reopened
        .download_to_path(&release, &destination, |bytes, total| {
            progress.push((bytes, total))
        })
        .await
        .unwrap();
    assert_eq!(progress.first(), Some(&(2, Some(4))));
    assert_eq!(progress.iter().map(|(bytes, _)| bytes).sum::<u64>(), 4);
    assert_eq!(std::fs::read(&destination).unwrap(), b"test");
    assert!(!destination.with_extension("part").exists());
    assert!(!destination.with_extension("signature").exists());
    server.await.unwrap();
}

#[tokio::test]
async fn resumed_download_never_publishes_unverified_or_misaligned_bytes() {
    for (response, saved, identity_matches, succeeds) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest",
            "te",
            true,
            true,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest",
            "old",
            false,
            true,
        ),
        (
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n",
            "test",
            true,
            true,
        ),
        (
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n",
            "oops",
            true,
            true,
        ),
        (
            "HTTP/1.1 206 Partial Content\r\nContent-Length: 2\r\nContent-Range: bytes 0-1/4\r\n\r\nst",
            "te",
            true,
            false,
        ),
        (
            "HTTP/1.1 206 Partial Content\r\nContent-Length: 2\r\nContent-Range: bytes 2-3/4\r\n\r\nxx",
            "te",
            true,
            false,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("1.0.1.bin");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (checker, release) =
            checker(&format!("http://{}/update", listener.local_addr().unwrap()));
        std::fs::write(destination.with_extension("part"), saved).unwrap();
        std::fs::write(
            destination.with_extension("signature"),
            if identity_matches {
                &release.signature
            } else {
                "old signature"
            },
        )
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, headers) = request(&listener).await;
            assert_eq!(headers.contains("range:"), identity_matches);
            socket.write_all(response.as_bytes()).await.unwrap();
            drop(socket);
            if response.starts_with("HTTP/1.1 416") && saved == "oops" {
                let (mut retry, headers) = request(&listener).await;
                assert!(!headers.contains("range:"));
                retry
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest")
                    .await
                    .unwrap();
            }
        });
        let result = checker
            .download_to_path(&release, &destination, |_, _| {})
            .await;
        assert_eq!(result.is_ok(), succeeds, "{response}: {result:?}");
        assert_eq!(destination.exists(), succeeds);
        if succeeds {
            assert_eq!(std::fs::read(&destination).unwrap(), b"test");
        }
        server.await.unwrap();
    }
}
