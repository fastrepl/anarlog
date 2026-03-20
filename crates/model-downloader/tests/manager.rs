use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use model_downloader::{DownloadableModel, Error, ModelDownloadManager, ModelDownloaderRuntime};

// --- test fixtures ---

struct TestRuntime {
    temp_dir: Arc<tempfile::TempDir>,
    progress_log: Arc<Mutex<Vec<(String, i8)>>>,
}

impl TestRuntime {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            temp_dir: Arc::new(tempfile::TempDir::new().unwrap()),
            progress_log: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn progress_values(&self) -> Vec<i8> {
        self.progress_log
            .lock()
            .unwrap()
            .iter()
            .map(|(_, p)| *p)
            .collect()
    }
}

impl ModelDownloaderRuntime<TestModel> for TestRuntime {
    fn models_base(&self) -> Result<PathBuf, Error> {
        Ok(self.temp_dir.path().to_path_buf())
    }

    fn emit_progress(&self, model: &TestModel, progress: i8) {
        self.progress_log
            .lock()
            .unwrap()
            .push((model.download_key(), progress));
    }
}

#[derive(Clone)]
struct TestModel {
    key: String,
    url: Option<String>,
    checksum: Option<u32>,
}

impl TestModel {
    fn with_url(key: &str, url: String) -> Self {
        Self {
            key: key.to_string(),
            url: Some(url),
            checksum: None,
        }
    }

    fn with_url_and_checksum(key: &str, url: String, checksum: u32) -> Self {
        Self {
            key: key.to_string(),
            url: Some(url),
            checksum: Some(checksum),
        }
    }

    fn without_url(key: &str) -> Self {
        Self {
            key: key.to_string(),
            url: None,
            checksum: None,
        }
    }
}

impl DownloadableModel for TestModel {
    fn download_key(&self) -> String {
        self.key.clone()
    }

    fn download_url(&self) -> Option<String> {
        self.url.clone()
    }

    fn download_checksum(&self) -> Option<u32> {
        self.checksum
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        models_base.join(format!("{}.bin", self.key))
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        Ok(self.download_destination(models_base).exists())
    }

    fn finalize_download(&self, _downloaded_path: &Path, _models_base: &Path) -> Result<(), Error> {
        Ok(())
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        std::fs::remove_file(self.download_destination(models_base)).map_err(Error::Io)
    }
}

// --- helpers ---

async fn start_mock_server(route: &str, body: Vec<u8>) -> MockServer {
    let server = MockServer::start().await;
    let len = body.len().to_string();

    Mock::given(method("HEAD"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", len.as_str()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .insert_header("content-length", len.as_str()),
        )
        .mount(&server)
        .await;

    server
}

async fn start_mock_server_with_delay(route: &str, body: Vec<u8>, delay: Duration) -> MockServer {
    let server = MockServer::start().await;
    let len = body.len().to_string();

    Mock::given(method("HEAD"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", len.as_str()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .insert_header("content-length", len.as_str())
                .set_delay(delay),
        )
        .mount(&server)
        .await;

    server
}

async fn wait_until_done(manager: &ModelDownloadManager<TestModel>, model: &TestModel) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if !manager.is_downloading(model).await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("download did not complete within 10s");
}

fn part_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_part = path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.contains(".part-"));
            if is_part {
                out.push(path);
            }
        }
    }
    out
}

// --- tests ---

#[tokio::test]
async fn model_path_returns_correct_path() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("my_model");

    let result = manager.model_path(&model).unwrap();

    assert_eq!(result, runtime.temp_dir.path().join("my_model.bin"));
}

#[tokio::test]
async fn is_downloaded_false_when_missing() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("absent");

    assert!(!manager.is_downloaded(&model).await.unwrap());
}

#[tokio::test]
async fn is_downloaded_true_when_file_exists() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("present");

    std::fs::write(manager.model_path(&model).unwrap(), b"weights").unwrap();

    assert!(manager.is_downloaded(&model).await.unwrap());
}

#[tokio::test]
async fn download_success() {
    let server = start_mock_server("/model.bin", b"fake weights".to_vec()).await;
    let url = format!("{}/model.bin", server.uri());

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::with_url("success", url);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;

    assert!(manager.is_downloaded(&model).await.unwrap());
    assert!(!manager.is_downloading(&model).await);

    let events = runtime.progress_values();
    assert!(events.contains(&0), "should emit 0 (started): {events:?}");
    assert!(
        events.contains(&100),
        "should emit 100 (finished): {events:?}"
    );
}

#[tokio::test]
async fn stale_cleanup_does_not_remove_replacement_download() {
    let slow_server =
        start_mock_server_with_delay("/model.bin", vec![1u8; 1024], Duration::from_millis(500))
            .await;
    let fast_server = start_mock_server("/model.bin", vec![2u8; 1024]).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let first = TestModel::with_url("replace", format!("{}/model.bin", slow_server.uri()));
    let second = TestModel::with_url("replace", format!("{}/model.bin", fast_server.uri()));

    manager.download(&first).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    manager.download(&second).await.unwrap();
    wait_until_done(&manager, &second).await;

    assert!(manager.is_downloaded(&second).await.unwrap());
    assert!(!manager.is_downloading(&second).await);
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
}

#[tokio::test]
async fn download_no_url_returns_error() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("no_url");

    let result = manager.download(&model).await;

    assert!(matches!(result, Err(Error::NoDownloadUrl(_))));
}

#[tokio::test]
async fn cancel_download_returns_false_when_idle() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("idle");

    let cancelled = manager.cancel_download(&model).await.unwrap();

    assert!(!cancelled);
}

#[tokio::test]
async fn cancel_download_returns_true_and_cleans_up() {
    let server = MockServer::start().await;

    Mock::given(method("HEAD"))
        .and(path("/slow.bin"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", "1024"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024])
                .set_delay(Duration::from_millis(500)),
        )
        .mount(&server)
        .await;

    let url = format!("{}/slow.bin", server.uri());
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::with_url("cancel_target", url);

    manager.download(&model).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(manager.is_downloading(&model).await);

    let cancelled = manager.cancel_download(&model).await.unwrap();

    assert!(cancelled);
    assert!(!manager.is_downloading(&model).await);
    assert!(!manager.is_downloaded(&model).await.unwrap());
    assert!(
        runtime.progress_values().contains(&-1),
        "should emit -1 on cancellation"
    );
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
}

#[tokio::test]
async fn download_failure_cleans_up_part_file() {
    let server = MockServer::start().await;

    Mock::given(method("HEAD"))
        .and(path("/fail.bin"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", "1024"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/fail.bin"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let url = format!("{}/fail.bin", server.uri());
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::with_url("fail", url);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;

    assert!(!manager.is_downloading(&model).await);
    assert!(!manager.is_downloaded(&model).await.unwrap());
    assert!(
        runtime.progress_values().contains(&-1),
        "should emit -1 on download failure"
    );
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
}

#[tokio::test]
async fn checksum_mismatch_cleans_up_part_file() {
    let body = b"checksum target".to_vec();
    let server = start_mock_server("/checksum.bin", body).await;
    let url = format!("{}/checksum.bin", server.uri());
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::with_url_and_checksum("checksum_fail", url, 123);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;

    assert!(!manager.is_downloading(&model).await);
    assert!(!manager.is_downloaded(&model).await.unwrap());
    assert!(
        runtime.progress_values().contains(&-1),
        "should emit -1 on checksum mismatch"
    );
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
}

#[tokio::test]
async fn cancel_download_immediately_after_start_returns_true() {
    let server =
        start_mock_server_with_delay("/slow.bin", vec![0u8; 1024], Duration::from_millis(500))
            .await;
    let url = format!("{}/slow.bin", server.uri());

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::with_url("immediate_cancel", url);

    manager.download(&model).await.unwrap();
    let cancelled = manager.cancel_download(&model).await.unwrap();

    assert!(cancelled);
    assert!(!manager.is_downloading(&model).await);
    assert!(
        runtime.progress_values().contains(&-1),
        "should emit -1 on cancellation"
    );
}

#[tokio::test]
async fn is_downloading_stays_true_during_replacement() {
    let slow_server =
        start_mock_server_with_delay("/model.bin", vec![1u8; 1024], Duration::from_millis(700))
            .await;
    let fast_server = start_mock_server("/model.bin", vec![2u8; 1024]).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let first = TestModel::with_url("replace_busy", format!("{}/model.bin", slow_server.uri()));
    let second = TestModel::with_url("replace_busy", format!("{}/model.bin", fast_server.uri()));

    manager.download(&first).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;

    let manager_for_task = manager.clone();
    let second_for_task = second.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let replacer = tokio::spawn(async move {
        let _ = started_tx.send(());
        manager_for_task.download(&second_for_task).await.unwrap();
    });
    started_rx.await.unwrap();

    for _ in 0..20 {
        assert!(manager.is_downloading(&second).await);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    replacer.await.unwrap();
    wait_until_done(&manager, &second).await;
    assert!(!manager.is_downloading(&second).await);
}

#[tokio::test]
async fn delete_success() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("to_delete");

    std::fs::write(manager.model_path(&model).unwrap(), b"weights").unwrap();
    assert!(manager.is_downloaded(&model).await.unwrap());

    manager.delete(&model).await.unwrap();

    assert!(!manager.is_downloaded(&model).await.unwrap());
}

#[tokio::test]
async fn delete_not_downloaded_returns_error() {
    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = TestModel::without_url("ghost");

    let result = manager.delete(&model).await;

    assert!(matches!(result, Err(Error::ModelNotDownloaded(_))));
}
