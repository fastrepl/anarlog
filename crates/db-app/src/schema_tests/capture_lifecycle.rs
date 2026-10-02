use super::*;

#[tokio::test]
async fn capture_lifecycle_marker_upsert_and_delete_are_conditional() {
    let db = test_db().await;
    let pool = db.pool();
    let original = r#"{"transcriptId":"transcript-1"}"#;
    let replacement = r#"{"transcriptId":"transcript-2"}"#;

    assert!(
        upsert_capture_lifecycle_marker(pool, "session-1", original, "transcript-1")
            .await
            .unwrap()
    );
    assert_eq!(
        get_capture_lifecycle_marker_json(pool, "session-1")
            .await
            .unwrap()
            .as_deref(),
        Some(original)
    );

    assert!(
        !upsert_capture_lifecycle_marker(pool, "session-1", replacement, "transcript-unrelated")
            .await
            .unwrap()
    );
    assert_eq!(
        get_capture_lifecycle_marker_json(pool, "session-1")
            .await
            .unwrap()
            .as_deref(),
        Some(original)
    );

    assert!(
        upsert_capture_lifecycle_marker(pool, "session-1", replacement, "transcript-1")
            .await
            .unwrap()
    );
    assert!(
        !delete_capture_lifecycle_marker(pool, "session-1", "transcript-1")
            .await
            .unwrap()
    );
    assert!(
        delete_capture_lifecycle_marker(pool, "session-1", "transcript-2")
            .await
            .unwrap()
    );
}
