use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub trait QueryEventSink: Clone + Send + 'static {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String>;
    fn send_error(&self, error: String) -> std::result::Result<(), String>;
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", content = "data")]
pub enum DependencyTarget {
    #[serde(rename = "table")]
    Table(String),
    #[serde(rename = "virtual_table")]
    VirtualTable(String),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", content = "data")]
pub enum DependencyAnalysis {
    #[serde(rename = "reactive")]
    Reactive {
        #[serde(
            serialize_with = "serialize_dependency_targets",
            deserialize_with = "deserialize_dependency_targets"
        )]
        targets: HashSet<DependencyTarget>,
    },
    #[serde(rename = "non_reactive")]
    NonReactive { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SubscriptionRegistration {
    pub id: String,
    pub analysis: DependencyAnalysis,
}

fn serialize_dependency_targets<S>(
    targets: &HashSet<DependencyTarget>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut targets = targets.iter().cloned().collect::<Vec<_>>();
    targets.sort();
    targets.serialize(serializer)
}

fn deserialize_dependency_targets<'de, D>(
    deserializer: D,
) -> Result<HashSet<DependencyTarget>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<DependencyTarget>::deserialize(deserializer).map(|targets| targets.into_iter().collect())
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use serde_json::json;

    use super::{DependencyAnalysis, DependencyTarget, SubscriptionRegistration};

    #[test]
    fn reactive_analysis_serializes_targets_in_stable_order() {
        let analysis = DependencyAnalysis::Reactive {
            targets: HashSet::from([
                DependencyTarget::VirtualTable("events_fts".to_string()),
                DependencyTarget::Table("sessions".to_string()),
                DependencyTarget::Table("events".to_string()),
            ]),
        };

        assert_eq!(
            serde_json::to_value(analysis).unwrap(),
            json!({
                "kind": "reactive",
                "data": {
                    "targets": [
                        { "kind": "table", "data": "events" },
                        { "kind": "table", "data": "sessions" },
                        { "kind": "virtual_table", "data": "events_fts" }
                    ]
                }
            })
        );
    }

    #[test]
    fn subscription_registration_round_trips_through_json() {
        let registration = SubscriptionRegistration {
            id: "sub-1".to_string(),
            analysis: DependencyAnalysis::NonReactive {
                reason: "unsupported source".to_string(),
            },
        };

        let value = serde_json::to_value(&registration).unwrap();
        let decoded: SubscriptionRegistration = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(decoded, registration);
        assert_eq!(
            value,
            json!({
                "id": "sub-1",
                "analysis": {
                    "kind": "non_reactive",
                    "data": {
                        "reason": "unsupported source"
                    }
                }
            })
        );
    }
}
