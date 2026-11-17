//! `settings/stats/{badges,badge-queries,badge-collection}`: "Your badges", their progress and
//! the collected state shipping keeps per owner in `app_settings`.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, OnceLock},
};

use anlg_db_execute::TransactionStatement;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use chrono_tz::Tz;
use desktop_runtime::{Result, RuntimeHandle, ServiceError};
use gpui::{RenderImage, SharedString, div, img, prelude::*, px};
use serde_json::{Value, json};

use super::Record;
use crate::ui::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Signup,
    Onboarding,
    Conversations,
    Weeks,
}

pub struct Badge {
    /// Persisted; keep stable when names or artwork change.
    pub id: &'static str,
    pub metric: Metric,
    pub target: u32,
    pub name: &'static str,
    pub description: &'static str,
    art: &'static [u8],
}

macro_rules! art {
    ($id:literal) => {
        include_bytes!(concat!(
            "../../../../desktop/public/assets/badges/",
            $id,
            ".webp"
        ))
    };
}

pub const BADGES: [Badge; 9] = [
    Badge {
        id: "hello",
        metric: Metric::Signup,
        target: 1,
        name: "Hello, Anarlog",
        description: "Create your Anarlog account. A place for your conversations to call home.",
        art: art!("hello"),
    },
    Badge {
        id: "all-set",
        metric: Metric::Onboarding,
        target: 1,
        name: "All Set",
        description: "Complete onboarding. You're ready for your next conversation.",
        art: art!("all-set"),
    },
    Badge {
        id: "first-words",
        metric: Metric::Conversations,
        target: 1,
        name: "First Words",
        description: "Capture your first conversation. Every collection starts somewhere.",
        art: art!("first-words"),
    },
    Badge {
        id: "good-listener",
        metric: Metric::Conversations,
        target: 10,
        name: "Good Listener",
        description: "Capture 10 conversations. More moments you can return to.",
        art: art!("good-listener"),
    },
    Badge {
        id: "memory-keeper",
        metric: Metric::Conversations,
        target: 50,
        name: "Memory Keeper",
        description: "Capture 50 conversations. A growing collection of ideas and decisions.",
        art: art!("memory-keeper"),
    },
    Badge {
        id: "story-collector",
        metric: Metric::Conversations,
        target: 100,
        name: "Story Collector",
        description: "Capture 100 conversations. A hundred stories, saved in your own words.",
        art: art!("story-collector"),
    },
    Badge {
        id: "living-library",
        metric: Metric::Conversations,
        target: 250,
        name: "Living Library",
        description: "Capture 250 conversations. Your own library of shared knowledge.",
        art: art!("living-library"),
    },
    Badge {
        id: "finding-rhythm",
        metric: Metric::Weeks,
        target: 4,
        name: "Finding Your Rhythm",
        description: "Capture conversations in 4 different weeks. They don't need to be consecutive.",
        art: art!("finding-rhythm"),
    },
    Badge {
        id: "familiar-face",
        metric: Metric::Weeks,
        target: 12,
        name: "Familiar Face",
        description: "Capture conversations in 12 different weeks. A little at a time, at your own pace.",
        art: art!("familiar-face"),
    },
];

/// `getBadgeProgress`: each badge's value, capped at its target.
pub fn progress(
    records: &[Record],
    signed_up: bool,
    onboarding_complete: bool,
    now: DateTime<Utc>,
    zone: Tz,
    monday: bool,
) -> Vec<u32> {
    let now_ms = now.timestamp_millis();
    let mut sessions = BTreeSet::new();
    let mut weeks = BTreeSet::new();
    for record in records {
        if record.is_demo || record.started_at > now_ms {
            continue;
        }
        let Some(date) = zone.timestamp_millis_opt(record.started_at).single() else {
            continue;
        };
        sessions.insert(record.session_id.as_str());
        let day = date.date_naive();
        let offset = if monday {
            day.weekday().num_days_from_monday()
        } else {
            day.weekday().num_days_from_sunday()
        };
        weeks.insert(day - Duration::days(offset.into()));
    }
    BADGES
        .iter()
        .map(|badge| {
            let value = match badge.metric {
                Metric::Signup => u32::from(signed_up),
                Metric::Onboarding => u32::from(onboarding_complete),
                Metric::Conversations => sessions.len() as u32,
                Metric::Weeks => weeks.len() as u32,
            };
            value.min(badge.target)
        })
        .collect()
}

fn prefix(owner: &str) -> String {
    format!("personal-badges.v1:{owner}:")
}

/// `parseCollectedBadges`: collected time by badge id; malformed rows are skipped.
pub fn parse_collected(rows: &[Value]) -> HashMap<&'static str, DateTime<Utc>> {
    let mut collected = HashMap::new();
    for row in rows {
        let Some(badge) = row["value_json"]
            .as_str()
            .and_then(|json| serde_json::from_str::<Value>(json).ok())
        else {
            continue;
        };
        let Some(definition) = BADGES.iter().find(|entry| badge["id"] == entry.id) else {
            continue;
        };
        if let Some(at) = badge["collectedAt"]
            .as_str()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        {
            collected.insert(definition.id, at.with_timezone(&Utc));
        }
    }
    collected
}

pub struct Collection {
    pub collected: HashMap<&'static str, DateTime<Utc>>,
    pub onboarding_complete: bool,
}

pub async fn load(runtime: &RuntimeHandle, owner: String) -> Result<Collection> {
    runtime
        .submit(move |services| async move {
            let prefix = prefix(&owner);
            let rows = services
                .executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE substr(id, 1, length(?)) = ?".into(),
                    vec![json!(prefix), json!(prefix)],
                )
                .await
                .map_err(failure)?;
            let onboarding = services
                .executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE id = ?".into(),
                    vec![json!(crate::product::onboarding::NEEDED_KEY)],
                )
                .await
                .map_err(failure)?;
            let stored = onboarding
                .first()
                .and_then(|row| row["value_json"].as_str())
                .and_then(|json| serde_json::from_str::<Value>(json).ok());
            Ok(Collection {
                collected: parse_collected(&rows),
                onboarding_complete: !crate::product::onboarding::needed(stored.as_ref()),
            })
        })?
        .receive()
        .await
}

/// `collectBadges`: a badge is collected once; later collections keep the first time.
pub async fn collect(runtime: &RuntimeHandle, owner: String, ids: Vec<&'static str>) -> Result<()> {
    let at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    runtime
        .submit(move |services| async move {
            let statements = ids
                .into_iter()
                .map(|id| TransactionStatement {
                    sql: "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?) ON CONFLICT(id) DO NOTHING".into(),
                    params: vec![
                        json!(format!("{}{id}", prefix(&owner))),
                        json!(json!({"id": id, "collectedAt": at}).to_string()),
                        json!(at),
                    ],
                    expected_rows_affected: None,
                })
                .collect();
            services
                .executor
                .execute_transaction(statements)
                .await
                .map(|_| ())
                .map_err(failure)
        })?
        .receive()
        .await
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

/// Shipping draws the line art with `mix-blend-multiply` (light) or `invert` plus
/// `mix-blend-screen` (dark). For grey art on white that equals ink whose alpha is the
/// darkness, black in light mode and white in dark mode.
fn emblem_image(art: &[u8], dark: bool) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory_with_format(art, image::ImageFormat::WebP).ok()?;
    let gray = image::imageops::resize(
        &decoded.to_luma8(),
        288,
        288,
        image::imageops::FilterType::Triangle,
    );
    let ink = if dark { 255 } else { 0 };
    let mut bgra = image::RgbaImage::new(gray.width(), gray.height());
    for (x, y, pixel) in gray.enumerate_pixels() {
        bgra.put_pixel(x, y, image::Rgba([ink, ink, ink, 255 - pixel[0]]));
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])))
}

type Emblems = HashMap<(&'static str, bool), Arc<RenderImage>>;

/// Decoded once per process, off the UI thread.
pub fn emblems() -> &'static OnceLock<Emblems> {
    static EMBLEMS: OnceLock<Emblems> = OnceLock::new();
    &EMBLEMS
}

pub fn decode_emblems() {
    emblems().get_or_init(|| {
        let mut decoded = HashMap::new();
        for badge in &BADGES {
            for dark in [false, true] {
                if let Some(image) = emblem_image(badge.art, dark) {
                    decoded.insert((badge.id, dark), image);
                }
            }
        }
        decoded
    });
}

/// `BadgeEmblem`.
pub fn emblem(id: &'static str, collected: bool, large: bool, colors: &Theme) -> gpui::Div {
    let size = if large { 144. } else { 96. };
    let dark = colors.background.l < 0.5;
    let art = emblems()
        .get()
        .and_then(|emblems| emblems.get(&(id, dark)).cloned());
    div()
        .relative()
        .flex_shrink_0()
        .size(px(size))
        .children(art.map(|art| {
            img(art)
                .size_full()
                .when(!collected, |image| image.opacity(0.3))
        }))
        .when(collected, |view| {
            view.child(
                div()
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .size_5()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.background)
                    .child(
                        gpui::svg()
                            .path("product/CheckIcon.svg")
                            .size_3()
                            .text_color(colors.foreground),
                    ),
            )
        })
}

pub fn progress_label(badge: &Badge, value: u32) -> SharedString {
    match badge.metric {
        Metric::Conversations => format!("{value} / {} conversations", badge.target).into(),
        Metric::Weeks => format!("{value} / {} active weeks", badge.target).into(),
        Metric::Signup => "Create your account".into(),
        Metric::Onboarding => "Complete onboarding".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn record(id: &str, day: &str, demo: bool) -> Record {
        let date: NaiveDate = day.parse().unwrap();
        Record {
            session_id: id.into(),
            started_at: Utc
                .from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap())
                .timestamp_millis(),
            duration_ms: 0,
            is_demo: demo,
        }
    }

    #[test]
    fn badge_progress_skips_the_demo_and_counts_distinct_weeks() {
        let now = Utc.with_ymd_and_hms(2026, 11, 17, 12, 0, 0).unwrap();
        let records = [
            record("a", "2026-11-16", false),
            record("a", "2026-11-16", false),
            record("b", "2026-11-02", false),
            record("demo", "2026-11-10", true),
            record("future", "2026-12-01", false),
        ];
        let values = progress(&records, false, true, now, Tz::UTC, true);
        let value = |id: &str| values[BADGES.iter().position(|badge| badge.id == id).unwrap()];
        assert_eq!(value("hello"), 0);
        assert_eq!(value("all-set"), 1);
        assert_eq!(value("first-words"), 1);
        assert_eq!(value("good-listener"), 2);
        assert_eq!(value("finding-rhythm"), 2);
    }

    #[test]
    fn collected_badges_keep_their_first_time_and_ignore_malformed_rows() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(async {
            ready.receive().await.unwrap();
            collect(&runtime, "owner".into(), vec!["hello"])
                .await
                .unwrap();
            let first = load(&runtime, "owner".into()).await.unwrap().collected["hello"];
            collect(&runtime, "owner".into(), vec!["hello", "all-set"])
                .await
                .unwrap();
            runtime
                .submit(|services| async move {
                    services
                        .executor
                        .execute(
                            "INSERT INTO app_settings (id, value_json) VALUES ('personal-badges.v1:owner:bad', 'nope')".into(),
                            vec![],
                        )
                        .await
                        .map(|_| ())
                        .map_err(failure)
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            let collection = load(&runtime, "owner".into()).await.unwrap();
            assert_eq!(collection.collected["hello"], first);
            assert_eq!(collection.collected.len(), 2);
            assert!(
                load(&runtime, "other".into())
                    .await
                    .unwrap()
                    .collected
                    .is_empty()
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
