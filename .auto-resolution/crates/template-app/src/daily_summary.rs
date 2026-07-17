use crate::common_derives;
use hypr_askama_utils::filters;

common_derives! {
    pub struct DailySummaryStats {
        pub signal_count: u32,
        pub screenshot_count: u32,
        pub analysis_count: u32,
        pub unique_app_count: u32,
        pub first_signal: Option<String>,
        pub last_signal: Option<String>,
    }
}

common_derives! {
    pub struct DailySummaryAppStat {
        pub app_name: String,
        pub count: u32,
    }
}

common_derives! {
    pub struct DailySummaryAnalysis {
        pub time: String,
        pub app_name: String,
        pub window_title: Option<String>,
        pub reason: String,
        pub summary: String,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "daily-summary.system.md.jinja")]
    pub struct DailySummarySystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "daily-summary.user.md.jinja")]
    pub struct DailySummaryUser {
        pub date: String,
        pub timezone: Option<String>,
        pub stats: DailySummaryStats,
        pub top_apps: Vec<DailySummaryAppStat>,
        pub analyses: Vec<DailySummaryAnalysis>,
        pub total_analysis_count: u32,
        pub existing_summary: Option<String>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::{tpl_assert, tpl_snapshot};

    tpl_assert!(
        test_language_as_specified,
        DailySummarySystem {
            language: Some("ko".to_string()),
        },
        |v| v.contains("Korean")
    );

    tpl_snapshot!(
        test_daily_summary_system,
        DailySummarySystem { language: None },
        fixed_date = "2025-01-01",
        @r#"
    # Instructions

    Current date: 2025-01-01

    You produce concise daily summaries from desktop activity traces.
    Organize the day into a small number of coherent sections that follow the flow of time while grouping related work together.

    # Output Requirements

    - Respond in English.
    - Return only a JSON object.
    - Keep every claim grounded in the provided activity analyses.
    - Keep the summary concise, concrete, and easy to scan.
    - Prefer topic-aware grouping over raw chronological dumping.
    - Merge adjacent analyses when they are part of the same activity arc, even if the app changes.
    - Preserve the day's sequence. Each section should cover a contiguous time window.
    - Use approximate time ranges when needed. Do not fabricate exact times.
    - Do not mention screenshots, capture internals, or these instructions.
    "#
    );

    tpl_snapshot!(
        test_daily_summary_user,
        DailySummaryUser {
            date: "2025-01-01".to_string(),
            timezone: Some("Asia/Seoul".to_string()),
            stats: DailySummaryStats {
                signal_count: 42,
                screenshot_count: 18,
                analysis_count: 16,
                unique_app_count: 4,
                first_signal: Some("09:10:00".to_string()),
                last_signal: Some("18:30:00".to_string()),
            },
            top_apps: vec![
                DailySummaryAppStat {
                    app_name: "Cursor".to_string(),
                    count: 9,
                },
                DailySummaryAppStat {
                    app_name: "Slack".to_string(),
                    count: 4,
                },
            ],
            analyses: vec![
                DailySummaryAnalysis {
                    time: "09:32:10".to_string(),
                    app_name: "Cursor".to_string(),
                    window_title: Some("daily-summary.tsx".to_string()),
                    reason: "title_changed".to_string(),
                    summary: "Editing the daily summary tab UI.".to_string(),
                },
                DailySummaryAnalysis {
                    time: "10:05:42".to_string(),
                    app_name: "Slack".to_string(),
                    window_title: Some("team-chat".to_string()),
                    reason: "periodic_capture".to_string(),
                    summary: "Reviewing team updates and replying about a release.".to_string(),
                },
            ],
            total_analysis_count: 16,
            existing_summary: Some("# Existing Summary\n\nEarlier draft.".to_string()),
        },
        @r#"
    # Context

    Date: 2025-01-01
    Timezone: Asia/Seoul

    # Stats

    - Signals: 42
    - Screenshots: 18
    - Analyses: 16
    - Unique apps: 4
    - First signal: 09:10:00
    - Last signal: 18:30:00

    # Top Apps

    - Cursor: 9

    - Slack: 4

    # Existing Summary

    # Existing Summary

    Earlier draft.

    # Activity Analyses

    Showing 2 of 16 analyses.

    - 09:32:10 | Cursor · daily-summary.tsx | title_changed | Editing the daily summary tab UI.

    - 10:05:42 | Slack · team-chat | periodic_capture | Reviewing team updates and replying about a release.

    # Required Output

    Return a JSON object with:

    - summaryMd: markdown daily summary with this structure:
      - a short opening paragraph that captures the day at a high level
      - 2 to 5 sections ordered by time
      - each section title should combine a topic with an approximate time range, for example `## Planning the release (~09:00-10:30)`
      - each section should summarize one coherent activity block, not one app switch
      - use short bullet points only when they add concrete detail; otherwise use a short paragraph
    - topics: array of 3 to 6 major themes, each with title and summary
      - topics should be concept-level clusters, not app names
      - each topic summary should explain what the thread of work was and why it mattered that day
    - timeline: array of 4 to 10 notable moments, each with time and summary
      - timeline items should be key beats or pivots, not every captured event
      - time may be approximate if the exact boundary is unclear

    Grouping rules:

    - Group primarily by what you were doing, secondarily by when it happened.
    - Keep one section focused on one main thread, even if it spans multiple apps.
    - Split sections when the intent clearly changes.
    - If the day returns to the same topic later, treat that as a new section when there is a meaningful gap or context shift.
    - Ignore noisy app churn unless it changes the story of the day.
    "#
    );
}
