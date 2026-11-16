use serde_json::Value;

pub fn step_configured(step: &Value) -> bool {
    match step["type"].as_str() {
        Some("markdown_export") => {
            let has_directory = step["directory"]
                .as_str()
                .is_some_and(|path| !path.trim().is_empty());
            let content = [
                "include_memo",
                "include_summary",
                "include_transcript",
                "include_action_items",
            ]
            .iter()
            .any(|key| step["options"][key].as_bool().unwrap_or(true));
            has_directory && content
        }
        Some("slack_recap" | "notion_update" | "linear_issues") => {
            step.get("target").is_some_and(Value::is_object)
        }
        _ => false,
    }
}

pub fn workflow_configured(workflow: &Value) -> bool {
    matches!(
        workflow["trigger"].as_str(),
        Some("note_enhanced" | "meeting_completed")
    ) && workflow["steps"]
        .as_array()
        .is_some_and(|steps| !steps.is_empty() && steps.iter().all(step_configured))
}

pub fn already_processed(workflow: &Value, session_id: &str) -> bool {
    workflow["processedSessionIds"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(session_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_export_false_options_differ_from_legacy_defaults() {
        let mut step = json!({"type":"markdown_export","directory":"/exports"});
        assert!(step_configured(&step));
        step["options"] = json!({"include_memo":false,"include_summary":false,"include_transcript":false,"include_action_items":false});
        assert!(!step_configured(&step));
        step["options"]["include_summary"] = json!(true);
        assert!(step_configured(&step));
        step["directory"] = json!(" ");
        assert!(!step_configured(&step));
    }

    #[test]
    fn every_step_must_be_configured_and_unknown_steps_are_not_executable() {
        let mut workflow = json!({"trigger":"meeting_completed","steps":[
            {"type":"slack_recap","target":{"id":"channel"}},
            {"type":"linear_issues","target":null}
        ],"processedSessionIds":["saved"]});
        assert!(!workflow_configured(&workflow));
        workflow["steps"][1]["target"] = json!({"id":"team"});
        assert!(workflow_configured(&workflow));
        assert!(already_processed(&workflow, "saved"));
        assert!(!already_processed(&workflow, "retryable"));
        workflow["steps"][1]["type"] = json!("unknown");
        assert!(!workflow_configured(&workflow));
    }
}
