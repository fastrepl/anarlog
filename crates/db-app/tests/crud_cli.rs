#![cfg(feature = "cli")]

use db_app::*;
use hypr_db_core2::Db3;

async fn setup() -> sqlx::SqlitePool {
    let db = Db3::connect_memory_plain().await.unwrap();
    migrate(db.pool()).await.unwrap();
    db.pool().clone()
}

mod org_cli_tests {
    use super::*;
    use db_app::organization_cli::{self, Commands, OutputFormat};

    #[tokio::test]
    async fn run_add_returns_valid_id() {
        let pool = setup().await;
        let id = organization_cli::add(&pool, "TestOrg").await.unwrap();
        assert!(!id.is_empty());

        let org = get_organization(&pool, &id).await.unwrap().unwrap();
        assert_eq!(org.name, "TestOrg");
    }

    #[tokio::test]
    async fn run_edit_updates_name() {
        let pool = setup().await;
        insert_organization(&pool, "org1", "Old").await.unwrap();

        organization_cli::edit(&pool, "org1", Some("New"))
            .await
            .unwrap();

        let org = get_organization(&pool, "org1").await.unwrap().unwrap();
        assert_eq!(org.name, "New");
    }

    #[tokio::test]
    async fn run_show_not_found() {
        let pool = setup().await;
        let err = organization_cli::show(&pool, "bad_id", OutputFormat::Text)
            .await
            .unwrap_err();
        assert!(err.message.contains("not found"), "error: {}", err.message);
    }

    #[tokio::test]
    async fn run_rm_uses_cascade_delete() {
        let pool = setup().await;
        insert_organization(&pool, "org1", "Acme").await.unwrap();
        insert_human(&pool, "h1", "Bob", "", "org1", "")
            .await
            .unwrap();

        organization_cli::rm(&pool, "org1").await.unwrap();
        assert!(get_organization(&pool, "org1").await.unwrap().is_none());

        // cascade: humans.org_id should be cleared
        let human = get_human(&pool, "h1").await.unwrap().unwrap();
        assert_eq!(human.org_id, "", "cascade should clear org_id");
    }

    #[tokio::test]
    async fn run_dispatches_none_to_list() {
        let pool = setup().await;
        insert_organization(&pool, "org1", "Acme").await.unwrap();
        // None command dispatches to list (pretty) — should not error
        organization_cli::run(&pool, None).await.unwrap();
    }

    #[tokio::test]
    async fn run_dispatches_add_variant() {
        let pool = setup().await;

        let cmd = Some(Commands::Add {
            name: "FromEnum".to_string(),
        });
        organization_cli::run(&pool, cmd).await.unwrap();

        let orgs = list_organizations(&pool).await.unwrap();
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].name, "FromEnum");
    }
}

mod human_cli_tests {
    use super::*;
    use db_app::human_cli::{self, Commands, OutputFormat};

    #[tokio::test]
    async fn add_with_optional_fields() {
        let pool = setup().await;

        let id = human_cli::add(&pool, "Alice", Some("a@b.com"), Some("org1"), Some("Eng"))
            .await
            .unwrap();

        let h = get_human(&pool, &id).await.unwrap().unwrap();
        assert_eq!(h.name, "Alice");
        assert_eq!(h.email, "a@b.com");
        assert_eq!(h.org_id, "org1");
        assert_eq!(h.job_title, "Eng");
    }

    #[tokio::test]
    async fn add_with_none_defaults_to_empty() {
        let pool = setup().await;

        let id = human_cli::add(&pool, "Bob", None, None, None)
            .await
            .unwrap();

        let h = get_human(&pool, &id).await.unwrap().unwrap();
        assert_eq!(h.name, "Bob");
        assert_eq!(h.email, "", "None should map to empty string");
        assert_eq!(h.org_id, "");
        assert_eq!(h.job_title, "");
    }

    #[tokio::test]
    async fn edit_partial_update() {
        let pool = setup().await;
        insert_human(&pool, "h1", "Alice", "a@b.com", "org1", "Eng")
            .await
            .unwrap();

        human_cli::edit(&pool, "h1", None, None, None, None, Some("notes"))
            .await
            .unwrap();

        let h = get_human(&pool, "h1").await.unwrap().unwrap();
        assert_eq!(h.name, "Alice", "unchanged fields preserved");
        assert_eq!(h.email, "a@b.com", "unchanged fields preserved");
        assert_eq!(h.memo, "notes");
    }

    #[tokio::test]
    async fn rm_cascades_to_meeting_participants() {
        let pool = setup().await;
        insert_human(&pool, "h1", "Alice", "", "", "")
            .await
            .unwrap();
        insert_meeting(&pool, "s1", None).await.unwrap();
        add_meeting_participant(&pool, "s1", "h1", "manual")
            .await
            .unwrap();

        human_cli::rm(&pool, "h1").await.unwrap();

        assert!(get_human(&pool, "h1").await.unwrap().is_none());
        let participants = list_meeting_participants(&pool, "s1").await.unwrap();
        assert!(
            participants.is_empty(),
            "cascade delete should remove participants"
        );
    }

    #[tokio::test]
    async fn show_json_does_not_error() {
        let pool = setup().await;
        insert_human(&pool, "h1", "Alice", "a@b.com", "", "")
            .await
            .unwrap();

        human_cli::show(&pool, "h1", OutputFormat::Json)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn run_full_lifecycle() {
        let pool = setup().await;

        // Add
        let add = Some(Commands::Add {
            name: "Test".to_string(),
            email: None,
            org_id: None,
            job_title: None,
        });
        human_cli::run(&pool, add).await.unwrap();

        let humans = list_humans(&pool).await.unwrap();
        assert_eq!(humans.len(), 1);
        let id = humans[0].id.clone();

        // Edit
        let edit = Some(Commands::Edit {
            id: id.clone(),
            name: Some("Updated".to_string()),
            email: None,
            org_id: None,
            job_title: None,
            memo: None,
        });
        human_cli::run(&pool, edit).await.unwrap();
        assert_eq!(
            get_human(&pool, &id).await.unwrap().unwrap().name,
            "Updated"
        );

        // Show
        let show = Some(Commands::Show {
            id: id.clone(),
            format: OutputFormat::Text,
        });
        human_cli::run(&pool, show).await.unwrap();

        // Rm
        let rm = Some(Commands::Rm { id: id.clone() });
        human_cli::run(&pool, rm).await.unwrap();
        assert!(get_human(&pool, &id).await.unwrap().is_none());
    }
}

mod serialize_tests {
    use super::*;

    #[tokio::test]
    async fn human_row_serializes_all_fields() {
        let pool = setup().await;
        insert_human(&pool, "h1", "Alice", "a@b.com", "", "Eng")
            .await
            .unwrap();

        let h = get_human(&pool, "h1").await.unwrap().unwrap();
        let json = serde_json::to_value(&h).unwrap();

        assert_eq!(json["id"], "h1");
        assert_eq!(json["name"], "Alice");
        assert_eq!(json["email"], "a@b.com");
        assert_eq!(json["job_title"], "Eng");
        assert_eq!(json["pinned"], false);
        assert!(json["linked_user_id"].is_null());
    }

    #[tokio::test]
    async fn organization_row_serializes_bool_correctly() {
        let pool = setup().await;
        insert_organization(&pool, "org1", "Acme").await.unwrap();

        let org = get_organization(&pool, "org1").await.unwrap().unwrap();
        let json = serde_json::to_value(&org).unwrap();

        assert_eq!(json["name"], "Acme");
        assert_eq!(json["pinned"], false);
        assert_eq!(json["id"], "org1");
    }

    #[tokio::test]
    async fn list_serializes_to_json_array() {
        let pool = setup().await;
        insert_organization(&pool, "org1", "Alpha").await.unwrap();
        insert_organization(&pool, "org2", "Beta").await.unwrap();

        let orgs = list_organizations(&pool).await.unwrap();
        let json = serde_json::to_value(&orgs).unwrap();

        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
    }
}
