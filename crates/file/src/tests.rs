use super::*;

#[test]
#[ignore]
fn test_calculate_file_size_and_checksum() {
    let base = "/Users/yujonglee/dev/anarlog/.cache";

    fn walk_dir(dir: &std::path::Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let metadata = std::fs::metadata(&path)?;
                let size = metadata.len();

                match calculate_file_checksum(&path) {
                    Ok(checksum) => {
                        println!(
                            "{} | Size: {} bytes | Checksum: {}",
                            path.display(),
                            size,
                            checksum
                        );
                    }
                    Err(e) => {
                        println!(
                            "{} | Size: {} bytes | Checksum: Error - {}",
                            path.display(),
                            size,
                            e
                        );
                    }
                }
            } else if path.is_dir() {
                if let Err(e) = walk_dir(&path) {
                    eprintln!("Error walking directory {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    let base_path = std::path::Path::new(base);
    if base_path.exists() {
        if let Err(e) = walk_dir(base_path) {
            eprintln!("Error walking base directory: {}", e);
        }
    } else {
        println!("Base directory does not exist: {}", base);
    }
}

#[tokio::test]
async fn test_request_with_range() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test-file"))
        .and(header("Range", "bytes=5-"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(b"CONTENT")
                .insert_header("Content-Range", "bytes 5-11/12"),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/test-file"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"FULL_CONTENT")
                .insert_header("Content-Length", "12"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/test-file", mock_server.uri());

    let full_response = request_with_range(&url, None).await.unwrap();
    assert_eq!(
        full_response.status().as_u16(),
        200,
        "Full request should return 200"
    );

    let range_response = request_with_range(&url, Some(5)).await.unwrap();
    assert_eq!(
        range_response.status().as_u16(),
        206,
        "Range request should return 206"
    );

    let content_range = range_response.headers().get("Content-Range").unwrap();
    assert_eq!(content_range.to_str().unwrap(), "bytes 5-11/12");
}

#[tokio::test]
async fn test_download_file_with_callback_mock() {
    use tempfile::NamedTempFile;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test-file"))
        .and(header("Range", "bytes=510-"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(b"SECOND_HALF".repeat(46))
                .insert_header("Content-Range", "bytes 510-1015/1016"),
        )
        .mount(&mock_server)
        .await;

    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();
    std::fs::write(temp_path, b"FIRST_HALF".repeat(51)).unwrap();

    let url = format!("{}/test-file", mock_server.uri());

    let range_response = request_with_range(&url, Some(510)).await.unwrap();
    assert_eq!(
        range_response.status().as_u16(),
        206,
        "Range request should return 206"
    );

    let result = download_file_with_callback(url.clone(), temp_path, |_| {}).await;

    assert!(result.is_ok());

    let content = std::fs::read(temp_path).unwrap();
    assert_eq!(content.len(), 1016);
    assert!(content.starts_with(b"FIRST_HALF"));
    assert!(content.ends_with(b"SECOND_HALF"));
}

#[tokio::test]
async fn test_download_file_with_callback_range_validation() {
    use tempfile::NamedTempFile;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test-file"))
        .and(header("Range", "bytes=5-"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"FULL_CONTENT")
                .insert_header("Content-Length", "12"),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/test-file"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"FULL_CONTENT")
                .insert_header("Content-Length", "12"),
        )
        .mount(&mock_server)
        .await;

    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    std::fs::write(temp_path, b"PARTIAL").unwrap();
    let initial_size = std::fs::metadata(temp_path).unwrap().len();
    assert_eq!(initial_size, 7);

    let url = format!("{}/test-file", mock_server.uri());

    let range_response = request_with_range(&url, Some(5)).await.unwrap();
    assert_eq!(
        range_response.status().as_u16(),
        200,
        "Server should return 200 when ignoring Range header"
    );

    let result = download_file_with_callback(url.clone(), temp_path, |_| {}).await;
    assert!(result.is_ok());

    let content = std::fs::read(temp_path).unwrap();
    assert_eq!(content, b"FULL_CONTENT");
    assert_eq!(content.len(), 12);
}

#[tokio::test]
#[ignore]
async fn test_download_file_with_callback_s3() {
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let s3_url = "https://storage2.hyprnote.com/v0/ggerganov/whisper.cpp/main/ggml-tiny-q8_0.bin";

    let partial_content = b"PARTIAL_CONTENT".repeat(100);
    std::fs::write(temp_path, &partial_content).unwrap();

    let initial_size = std::fs::metadata(temp_path).unwrap().len();
    assert_eq!(initial_size, 1500);

    let range_response = request_with_range(s3_url, Some(initial_size))
        .await
        .unwrap();
    assert_eq!(
        range_response.status().as_u16(),
        206,
        "Server should respond with 206 for range requests"
    );

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let progress_events_clone = Arc::clone(&progress_events);

    let result = download_file_with_callback(s3_url, temp_path, |progress| {
        progress_events_clone.lock().unwrap().push(progress);
    })
    .await;

    assert!(result.is_ok());

    let file_size = std::fs::metadata(temp_path).unwrap().len();
    assert!(
        file_size > initial_size,
        "File should have grown from resume"
    );

    let events = progress_events.lock().unwrap();
    assert!(
        !events.is_empty(),
        "Progress events should have been recorded"
    );
}

#[tokio::test]
#[ignore]
async fn test_download_file_parallel_mock() {
    use std::time::Instant;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let large_content = vec![0u8; 1024 * 1024 * 1024];
    let content_length = large_content.len();

    Mock::given(method("HEAD"))
        .and(path("/large-file"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", content_length.to_string().as_str())
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&mock_server)
        .await;

    let expected_chunk_size = DEFAULT_CHUNK_SIZE as usize;

    for chunk_start in (0..content_length).step_by(expected_chunk_size) {
        let chunk_end = std::cmp::min(chunk_start + expected_chunk_size - 1, content_length - 1);
        let chunk_data = large_content[chunk_start..=chunk_end].to_vec();
        let range_header = format!("bytes={}-{}", chunk_start, chunk_end);
        let content_range = format!("bytes {}-{}/{}", chunk_start, chunk_end, content_length);

        Mock::given(method("GET"))
            .and(path("/large-file"))
            .and(header("Range", range_header.as_str()))
            .respond_with(
                ResponseTemplate::new(206)
                    .set_body_bytes(chunk_data)
                    .insert_header("Content-Range", content_range.as_str()),
            )
            .mount(&mock_server)
            .await;
    }

    Mock::given(method("GET"))
        .and(path("/large-file"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(large_content.clone())
                .insert_header("Content-Length", content_length.to_string().as_str()),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let url = format!("{}/large-file", mock_server.uri());

    let test_client = reqwest::Client::builder().http1_only().build().unwrap();

    let head_response = test_client
        .head(&url)
        .header("User-Agent", "curl/8.14.1")
        .header("Accept", "*/*")
        .send()
        .await
        .unwrap();

    let file_size = get_content_length_from_headers(&head_response).unwrap_or(0);

    let supports_ranges = head_response
        .headers()
        .get("accept-ranges")
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("")
        == "bytes";
    assert!(file_size > 0, "File size should be greater than 0");

    println!(
        "Server supports ranges: {}, File size: {} MB",
        supports_ranges,
        file_size / 1024 / 1024
    );

    let temp_file1 = NamedTempFile::new().unwrap();
    let start = Instant::now();
    download_file_with_callback(&url, temp_file1.path(), |_| {})
        .await
        .unwrap();
    let serial_duration = start.elapsed();

    let temp_file2 = NamedTempFile::new().unwrap();
    let start = Instant::now();
    download_file_parallel(&url, temp_file2.path(), |_| {})
        .await
        .unwrap();
    let parallel_duration = start.elapsed();

    println!(
        "Serial: {:?}, Parallel: {:?}",
        serial_duration, parallel_duration
    );
    let speedup = serial_duration.as_secs_f64() / parallel_duration.as_secs_f64();
    println!("Speedup: {:.2}x", speedup);

    let serial_size = std::fs::metadata(temp_file1.path()).unwrap().len();
    let parallel_size = std::fs::metadata(temp_file2.path()).unwrap().len();
    assert_eq!(
        serial_size, parallel_size,
        "Both downloads should produce files of the same size"
    );
    assert_eq!(
        serial_size, content_length as u64,
        "Downloaded file should match expected size"
    );

    assert!(
        speedup >= 1.1,
        "Parallel download should be at least 10% faster: serial={:?}, parallel={:?}, speedup={:.2}x",
        serial_duration,
        parallel_duration,
        speedup
    );
}

#[tokio::test]
#[ignore]
async fn test_download_file_parallel_s3() {
    use std::time::Instant;
    use tempfile::NamedTempFile;

    let url = "https://storage2.hyprnote.com/v0/yujonglee/hypr-llm-sm/model_q4_k_m.gguf";
    let test_client = reqwest::Client::builder().http1_only().build().unwrap();

    let head_response = test_client
        .head(url)
        .header("User-Agent", "curl/8.14.1")
        .header("Accept", "*/*")
        .send()
        .await
        .unwrap();

    let file_size = get_content_length_from_headers(&head_response).unwrap_or(0);

    let supports_ranges = head_response
        .headers()
        .get("accept-ranges")
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("")
        == "bytes";
    assert!(file_size > 0, "File size should be greater than 0");

    println!(
        "Server supports ranges: {}, File size: {} MB",
        supports_ranges,
        file_size / 1024 / 1024
    );

    let temp_file1 = NamedTempFile::new().unwrap();
    let start = Instant::now();
    download_file_with_callback(url, temp_file1.path(), {
        use std::cell::RefCell;
        let last_percent = RefCell::new(0u8);
        move |progress| match progress {
            DownloadProgress::Started => println!("Serial download started"),
            DownloadProgress::Progress(downloaded, total) => {
                let percent = (downloaded as f64 / total as f64 * 100.0) as u8;
                let mut last = last_percent.borrow_mut();
                if percent >= *last + 10 {
                    println!(
                        "Serial download: {}% ({}/{} bytes)",
                        percent, downloaded, total
                    );
                    *last = percent;
                }
            }
            DownloadProgress::Finished => println!("Serial download finished"),
        }
    })
    .await
    .unwrap();
    let serial_duration = start.elapsed();

    let temp_file2 = NamedTempFile::new().unwrap();
    let start = Instant::now();
    download_file_parallel(url, temp_file2.path(), {
        use std::sync::{Arc, Mutex};
        let last_percent = Arc::new(Mutex::new(0u8));
        move |progress| match progress {
            DownloadProgress::Started => println!("Parallel download started"),
            DownloadProgress::Progress(downloaded, total) => {
                let percent = (downloaded as f64 / total as f64 * 100.0) as u8;
                let mut last = last_percent.lock().unwrap();
                if percent >= *last + 10 {
                    println!(
                        "Parallel download: {}% ({}/{} bytes)",
                        percent, downloaded, total
                    );
                    *last = percent;
                }
            }
            DownloadProgress::Finished => println!("Parallel download finished"),
        }
    })
    .await
    .unwrap();
    let parallel_duration = start.elapsed();

    println!(
        "Serial: {:?}, Parallel: {:?}",
        serial_duration, parallel_duration
    );
    let speedup = serial_duration.as_secs_f64() / parallel_duration.as_secs_f64();
    println!("Speedup: {:.2}x", speedup);

    let serial_size = std::fs::metadata(temp_file1.path()).unwrap().len();
    let parallel_size = std::fs::metadata(temp_file2.path()).unwrap().len();
    assert_eq!(
        serial_size, parallel_size,
        "Both downloads should produce files of the same size"
    );

    assert!(
        speedup >= 1.1,
        "Parallel download should be at least 10% faster: serial={:?}, parallel={:?}, speedup={:.2}x",
        serial_duration,
        parallel_duration,
        speedup
    );
}
