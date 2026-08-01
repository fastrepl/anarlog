use super::*;

#[tokio::test]
async fn schedules_exact_delete_identity_with_stable_replay_shape_without_storage() {
    let delete_not_before = timestamp_after(24 * 60 * 60);
    let created_server = MockServer::start().await;
    mount_rpc(
        &created_server,
        "schedule_attachment_backup_deletion",
        ResponseTemplate::new(200)
            .set_body_json(json!([scheduled_deletion_row(&delete_not_before, true)])),
    )
    .await;

    let created_response = test_router(&created_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(created_response.status(), StatusCode::OK);
    let created_body = response_json(created_response).await;
    assert_eq!(
        created_body,
        json!({
            "objectKey": object_key(),
            "attachmentRef": ATTACHMENT_REF,
            "versionRef": VERSION_REF,
            "deleteRequestId": DELETE_REQUEST_ID,
            "deleteFenceId": DELETE_FENCE_ID,
            "deleteGeneration": 7,
            "deleteNotBefore": delete_not_before
        })
    );
    let created_requests = created_server.received_requests().await.unwrap();
    assert_eq!(created_requests.len(), 1);
    assert_eq!(
        created_requests[0].url.path(),
        "/rest/v1/rpc/schedule_attachment_backup_deletion"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&created_requests[0].body).unwrap(),
        json!({
            "p_owner_user_id": OWNER,
            "p_attachment_ref": ATTACHMENT_REF,
            "p_version_ref": VERSION_REF,
            "p_object_key": object_key(),
            "p_delete_request_id": DELETE_REQUEST_ID
        })
    );

    let replay_server = MockServer::start().await;
    mount_rpc(
        &replay_server,
        "schedule_attachment_backup_deletion",
        ResponseTemplate::new(200)
            .set_body_json(json!([scheduled_deletion_row(&delete_not_before, false)])),
    )
    .await;
    let replay_response = test_router(&replay_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(replay_response.status(), StatusCode::OK);
    assert_eq!(response_json(replay_response).await, created_body);
    let replay_requests = replay_server.received_requests().await.unwrap();
    assert_eq!(replay_requests.len(), 1);
    assert!(
        replay_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );

    let dependency_server = MockServer::start().await;
    mount_rpc(
        &dependency_server,
        "cancel_attachment_backup_deletion",
        ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "dependency_appeared",
            "object_key": object_key(),
            "was_cancelled": false
        }])),
    )
    .await;
    let dependency = test_router(&dependency_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete/cancel",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(dependency.status(), StatusCode::OK);
    assert_eq!(
        response_json(dependency).await,
        json!({
            "objectKey": object_key(),
            "attachmentRef": ATTACHMENT_REF,
            "versionRef": VERSION_REF,
            "deleteRequestId": DELETE_REQUEST_ID
        })
    );
    let dependency_requests = dependency_server.received_requests().await.unwrap();
    assert_eq!(dependency_requests.len(), 1);
    assert!(
        dependency_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );
}

#[tokio::test]
async fn distinguishes_delete_outcomes_from_generic_conflicts_without_storage() {
    let dependency_server = MockServer::start().await;
    mount_rpc(
        &dependency_server,
        "schedule_attachment_backup_deletion",
        ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "dependency_appeared",
            "object_id": null,
            "object_key": object_key(),
            "delete_request_id": DELETE_REQUEST_ID,
            "delete_fence_id": null,
            "delete_generation": null,
            "delete_not_before": null,
            "was_created": false
        }])),
    )
    .await;
    let dependency = test_router(&dependency_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(dependency.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(dependency).await["error"]["code"],
        "attachment_backup_dependency_appeared"
    );
    let dependency_requests = dependency_server.received_requests().await.unwrap();
    assert_eq!(dependency_requests.len(), 1);
    assert!(
        dependency_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );

    let cancelled_server = MockServer::start().await;
    mount_rpc(
        &cancelled_server,
        "schedule_attachment_backup_deletion",
        ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "cancelled",
            "object_id": null,
            "object_key": object_key(),
            "delete_request_id": DELETE_REQUEST_ID,
            "delete_fence_id": null,
            "delete_generation": null,
            "delete_not_before": null,
            "was_created": false
        }])),
    )
    .await;
    let cancelled = test_router(&cancelled_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(cancelled).await["error"]["code"],
        "attachment_backup_delete_cancelled"
    );
    let cancelled_requests = cancelled_server.received_requests().await.unwrap();
    assert_eq!(cancelled_requests.len(), 1);
    assert!(
        cancelled_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );

    let conflict_server = MockServer::start().await;
    mount_rpc(
        &conflict_server,
        "schedule_attachment_backup_deletion",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "40001",
            "message": "delete identity changed"
        })),
    )
    .await;
    let conflict = test_router(&conflict_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(conflict).await["error"]["code"],
        "attachment_backup_conflict"
    );
    let conflict_requests = conflict_server.received_requests().await.unwrap();
    assert_eq!(conflict_requests.len(), 1);
    assert!(
        conflict_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );
}

#[tokio::test]
async fn cancels_exact_delete_identity_with_a_stable_idempotent_shape() {
    let created_server = MockServer::start().await;
    mount_rpc(
        &created_server,
        "cancel_attachment_backup_deletion",
        ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "cancelled",
            "object_key": object_key(),
            "was_cancelled": true
        }])),
    )
    .await;
    let created = test_router(&created_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete/cancel",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_body = response_json(created).await;
    assert_eq!(
        created_body,
        json!({
            "objectKey": object_key(),
            "attachmentRef": ATTACHMENT_REF,
            "versionRef": VERSION_REF,
            "deleteRequestId": DELETE_REQUEST_ID
        })
    );
    let requests = created_server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.path(),
        "/rest/v1/rpc/cancel_attachment_backup_deletion"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        json!({
            "p_owner_user_id": OWNER,
            "p_attachment_ref": ATTACHMENT_REF,
            "p_version_ref": VERSION_REF,
            "p_object_key": object_key(),
            "p_delete_request_id": DELETE_REQUEST_ID
        })
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );

    let replay_server = MockServer::start().await;
    mount_rpc(
        &replay_server,
        "cancel_attachment_backup_deletion",
        ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "cancelled",
            "object_key": object_key(),
            "was_cancelled": false
        }])),
    )
    .await;
    let replay = test_router(&replay_server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete/cancel",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(response_json(replay).await, created_body);
    let replay_requests = replay_server.received_requests().await.unwrap();
    assert_eq!(replay_requests.len(), 1);
    assert!(
        replay_requests
            .iter()
            .all(|request| !request.url.path().contains("/storage/v1/"))
    );
}

#[tokio::test]
async fn reports_delete_cancellation_after_gc_as_too_late() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "cancel_attachment_backup_deletion",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "55006",
            "message": "gc already owns the object"
        })),
    )
    .await;
    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete/cancel",
            delete_request_body(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "attachment_backup_delete_too_late"
    );
}
