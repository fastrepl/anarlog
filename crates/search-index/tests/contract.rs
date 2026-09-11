use search_index::{
    CollectionConfig, Collections, DEFAULT_COLLECTION, SCHEMA_VERSION, build_schema, contract,
};

#[tokio::test]
async fn fixed_documents_cover_search_requests_and_snippets() {
    let temp = tempfile::tempdir().unwrap();
    let collections = Collections::default();
    collections
        .register_collection(
            temp.path(),
            CollectionConfig {
                name: DEFAULT_COLLECTION.into(),
                path: "default".into(),
                schema_builder: build_schema,
                schema_version: SCHEMA_VERSION,
            },
        )
        .await
        .unwrap();

    for document in contract::documents() {
        collections.add_document(None, document).await.unwrap();
    }
    collections.reload(None).await.unwrap();

    let requests = contract::requests();
    let expected_ids = [
        vec!["session-2", "session-1"],
        vec!["session-1"],
        vec!["session-3"],
        vec!["session-5"],
        vec!["session-2", "session-1"],
        vec![],
    ];

    for (index, (request, expected)) in requests.into_iter().zip(expected_ids).enumerate() {
        let result = collections.search(request).await.unwrap();
        let ids: Vec<&str> = result
            .hits
            .iter()
            .map(|hit| hit.document.id.as_str())
            .collect();
        assert_eq!(ids, expected);
        if !expected.is_empty() {
            assert!(
                result.hits.iter().any(|hit| {
                    hit.title_snippet
                        .as_ref()
                        .is_some_and(|snippet| !snippet.highlights.is_empty())
                        || hit
                            .content_snippet
                            .as_ref()
                            .is_some_and(|snippet| !snippet.highlights.is_empty())
                }),
                "at least one hit should include a highlight"
            );
        }
        if index == 0 {
            assert_eq!(
                (
                    result.hits[0].title_snippet.as_ref().unwrap().highlights[0].start,
                    result.hits[0].title_snippet.as_ref().unwrap().highlights[0].end,
                ),
                (0, 1)
            );
            assert_eq!(
                (
                    result.hits[0].content_snippet.as_ref().unwrap().highlights[0].start,
                    result.hits[0].content_snippet.as_ref().unwrap().highlights[0].end,
                ),
                (0, 1)
            );
        }
        if index == 3 {
            assert_eq!(
                (
                    result.hits[0].title_snippet.as_ref().unwrap().highlights[0].start,
                    result.hits[0].title_snippet.as_ref().unwrap().highlights[0].end,
                ),
                (0, 3)
            );
        }
    }
}
