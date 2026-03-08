use supabase_storage::SupabaseStorage;

fn client_from_env() -> SupabaseStorage {
    let url = std::env::var("SUPABASE_URL").expect("SUPABASE_URL must be set");
    let key =
        std::env::var("SUPABASE_SERVICE_ROLE_KEY").expect("SUPABASE_SERVICE_ROLE_KEY must be set");
    SupabaseStorage::new(reqwest::Client::new(), &url, &key)
}

#[tokio::test]
#[ignore]
async fn create_signed_url() {
    let storage = client_from_env();
    let bucket = std::env::var("TEST_BUCKET").unwrap_or("test".into());
    let path = std::env::var("TEST_OBJECT_PATH").unwrap_or("test.txt".into());

    let url = storage.create_signed_url(&bucket, &path, 60).await.unwrap();

    assert!(url.starts_with("http"));
}

#[tokio::test]
#[ignore]
async fn create_signed_url_nonexistent_object() {
    let storage = client_from_env();

    let result = storage
        .create_signed_url("nonexistent-bucket", "no-such-file.txt", 60)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("failed to create signed URL"));
}

#[tokio::test]
#[ignore]
async fn delete_file_nonexistent() {
    let storage = client_from_env();

    let result = storage
        .delete_file("nonexistent-bucket", "no-such-file.txt")
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("failed to delete file"));
}

#[tokio::test]
#[ignore]
async fn create_signed_url_bad_credentials() {
    let url = std::env::var("SUPABASE_URL").expect("SUPABASE_URL must be set");
    let storage = SupabaseStorage::new(reqwest::Client::new(), &url, "invalid-key");

    let result = storage
        .create_signed_url("any-bucket", "any-path.txt", 60)
        .await;

    assert!(result.is_err());
}
