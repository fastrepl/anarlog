use crate::{CreatedAtFilter, SearchDocument, SearchFilters, SearchOptions, SearchRequest};

pub fn documents() -> Vec<SearchDocument> {
    vec![
        document("session-1", "Kickoff", "Discuss pro action items", 1_000),
        document(
            "session-2",
            "Pro review",
            "Pro pro plan and milestones",
            2_000,
        ),
        document(
            "session-3",
            "Customer call",
            "Discuss onboarding and mee",
            3_000,
        ),
        document("session-4", "Release planning", "Release checklist", 4_000),
        document(
            "session-5",
            "プロジェクト会議",
            "東京チームの会議メモ",
            5_000,
        ),
        document(
            "session-6",
            "고객 미팅",
            "프로젝트 일정과 액션 아이템",
            6_000,
        ),
        document("session-7", "Design notes", "A quiet note", 7_000),
        document("session-8", "Weekly sync", "Weekly follow-up", 8_000),
    ]
}

pub fn requests() -> Vec<SearchRequest> {
    vec![
        request("pro", SearchFilters::default(), 100, true),
        request("\"pro act\"", SearchFilters::default(), 100, true),
        request(
            "mee",
            SearchFilters {
                created_at: Some(CreatedAtFilter {
                    gte: Some(3_000),
                    lte: Some(6_000),
                    ..Default::default()
                }),
                ..Default::default()
            },
            100,
            true,
        ),
        request("プロ", SearchFilters::default(), 100, true),
        request("pro", SearchFilters::default(), 2, true),
        request(
            "term-that-does-not-exist",
            SearchFilters::default(),
            100,
            true,
        ),
    ]
}

fn document(id: &str, title: &str, content: &str, created_at: i64) -> SearchDocument {
    SearchDocument {
        id: id.into(),
        doc_type: "session".into(),
        language: None,
        title: title.into(),
        content: content.into(),
        created_at,
        facets: Vec::new(),
    }
}

fn request(query: &str, filters: SearchFilters, limit: usize, snippets: bool) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        collection: None,
        filters,
        limit,
        options: SearchOptions {
            fuzzy: Some(false),
            snippets: Some(snippets),
            ..Default::default()
        },
    }
}
