use std::ops::Bound;

use tantivy::query::{Query, RangeQuery};
use tantivy::schema::Field;
use tantivy::{DateTime, Term};

use crate::CreatedAtFilter;

fn to_date_time(timestamp_millis: i64) -> DateTime {
    DateTime::from_timestamp_millis(timestamp_millis)
}

fn date_term(field: Field, timestamp_millis: i64) -> Term {
    Term::from_field_date(field, to_date_time(timestamp_millis))
}

pub fn build_created_at_range_query(
    field: Field,
    filter: &CreatedAtFilter,
) -> Option<Box<dyn Query>> {
    if let Some(eq) = filter.eq {
        Some(Box::new(RangeQuery::new(
            Bound::Included(date_term(field, eq)),
            Bound::Included(date_term(field, eq)),
        )))
    } else {
        let lower = match (filter.gte, filter.gt) {
            (Some(gte), _) => Bound::Included(date_term(field, gte)),
            (None, Some(gt)) => Bound::Excluded(date_term(field, gt)),
            (None, None) => Bound::Unbounded,
        };
        let upper = match (filter.lte, filter.lt) {
            (Some(lte), _) => Bound::Included(date_term(field, lte)),
            (None, Some(lt)) => Bound::Excluded(date_term(field, lt)),
            (None, None) => Bound::Unbounded,
        };

        if matches!(lower, Bound::Unbounded) && matches!(upper, Bound::Unbounded) {
            return None;
        }

        Some(Box::new(RangeQuery::new(lower, upper)))
    }
}

#[cfg(test)]
mod tests {
    use tantivy::collector::Count;
    use tantivy::collector::TopDocs;
    use tantivy::query::{AllQuery, BooleanQuery, Occur, QueryParser};
    use tantivy::schema::{DateOptions, DateTimePrecision, FAST, STORED, STRING, Schema};
    use tantivy::{Index, TantivyDocument};

    use super::*;

    fn search_count(filter: CreatedAtFilter) -> usize {
        let mut schema_builder = Schema::builder();
        let created_at = schema_builder.add_date_field(
            "created_at",
            DateOptions::from(FAST | STORED).set_precision(DateTimePrecision::Milliseconds),
        );
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);
        let mut writer = index.writer(20_000_000).unwrap();

        for timestamp in [100_i64, 101, 250, 251] {
            let mut doc = TantivyDocument::new();
            doc.add_date(created_at, DateTime::from_timestamp_millis(timestamp));
            writer.add_document(doc).unwrap();
        }

        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let query = build_created_at_range_query(created_at, &filter).unwrap();

        searcher.search(&*query, &Count).unwrap()
    }

    #[test]
    fn build_created_at_range_query_matches_exact_millisecond() {
        assert_eq!(
            search_count(CreatedAtFilter {
                eq: Some(101),
                ..Default::default()
            }),
            1
        );
    }

    #[test]
    fn build_created_at_range_query_respects_exclusive_and_inclusive_bounds() {
        assert_eq!(
            search_count(CreatedAtFilter {
                gt: Some(100),
                lte: Some(250),
                ..Default::default()
            }),
            2
        );
    }

    #[test]
    fn build_created_at_range_query_supports_one_sided_lower_bound() {
        assert_eq!(
            search_count(CreatedAtFilter {
                gte: Some(250),
                ..Default::default()
            }),
            2
        );
    }

    #[test]
    fn build_created_at_range_query_supports_one_sided_upper_bound() {
        assert_eq!(
            search_count(CreatedAtFilter {
                lt: Some(250),
                ..Default::default()
            }),
            2
        );
    }

    #[test]
    fn build_created_at_range_query_filters_text_search_results() {
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", STRING | STORED);
        let created_at = schema_builder.add_date_field(
            "created_at",
            DateOptions::from(FAST | STORED).set_precision(DateTimePrecision::Milliseconds),
        );
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);
        let mut writer = index.writer(20_000_000).unwrap();

        for (title_value, timestamp) in [
            ("meeting", 100_i64),
            ("meeting", 200_i64),
            ("note", 200_i64),
        ] {
            let mut doc = TantivyDocument::new();
            doc.add_text(title, title_value);
            doc.add_date(created_at, DateTime::from_timestamp_millis(timestamp));
            writer.add_document(doc).unwrap();
        }

        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let text_query = QueryParser::for_index(&index, vec![title])
            .parse_query("meeting")
            .unwrap();
        let date_query = build_created_at_range_query(
            created_at,
            &CreatedAtFilter {
                gte: Some(150),
                lte: Some(250),
                ..Default::default()
            },
        )
        .unwrap();

        let combined_query =
            BooleanQuery::new(vec![(Occur::Must, text_query), (Occur::Must, date_query)]);
        let hits = searcher
            .search(&combined_query, &TopDocs::with_limit(10))
            .unwrap();

        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn build_created_at_range_query_filters_all_query_results() {
        let mut schema_builder = Schema::builder();
        let created_at = schema_builder.add_date_field(
            "created_at",
            DateOptions::from(FAST | STORED).set_precision(DateTimePrecision::Milliseconds),
        );
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);
        let mut writer = index.writer(20_000_000).unwrap();

        for timestamp in [100_i64, 200, 300] {
            let mut doc = TantivyDocument::new();
            doc.add_date(created_at, DateTime::from_timestamp_millis(timestamp));
            writer.add_document(doc).unwrap();
        }

        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let date_query = build_created_at_range_query(
            created_at,
            &CreatedAtFilter {
                gte: Some(150),
                lte: Some(300),
                ..Default::default()
            },
        )
        .unwrap();

        let combined_query = BooleanQuery::new(vec![
            (Occur::Must, Box::new(AllQuery)),
            (Occur::Must, date_query),
        ]);
        let hits = searcher
            .search(&combined_query, &TopDocs::with_limit(10))
            .unwrap();

        assert_eq!(hits.len(), 2);
    }
}
