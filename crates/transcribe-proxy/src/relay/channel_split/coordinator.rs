use crate::relay::types::ShutdownSignal;

use super::payload::{FinalizeMode, RewrittenSplitResponse};

#[derive(Debug)]
pub(super) enum SplitEvent {
    FinalizeRequested,
    Text {
        channel: usize,
        raw: String,
    },
    UpstreamClosed {
        channel: usize,
        code: u16,
        reason: String,
    },
    Fatal(ShutdownSignal),
    ClientClosed,
}

pub(super) enum CoordinatorAction {
    ForwardText(String),
    ForwardRewritten(RewrittenSplitResponse),
    CloseDownstream { code: u16, reason: String },
    AbortDownstream,
    ShutdownUpstreams(ShutdownSignal),
}

#[derive(Clone, Copy, Default)]
struct SplitChannelState {
    closed: bool,
    last_finalize_generation: Option<u64>,
}

struct PendingFinalize {
    channel: usize,
    generation: u64,
    response: RewrittenSplitResponse,
}

#[derive(Default)]
pub(super) struct SplitCoordinator {
    channels: [SplitChannelState; 2],
    pending_finalize: Option<PendingFinalize>,
    current_finalize_generation: u64,
    terminal_finalize_generation: Option<u64>,
}

impl SplitCoordinator {
    pub(super) fn handle_finalize_requested(&mut self) {
        self.current_finalize_generation += 1;
    }

    pub(super) fn handle_text(
        &mut self,
        channel: usize,
        rewritten: Option<RewrittenSplitResponse>,
        passthrough_text: Option<String>,
        upstream_error: Option<(u16, String)>,
    ) -> Vec<CoordinatorAction> {
        let mut actions = Vec::new();

        if upstream_error.is_some() {
            self.flush_pending(&mut actions, FinalizeMode::NonTerminal);
        }

        if let Some(rewritten) = rewritten {
            if rewritten.had_finalize() {
                self.channels[channel].last_finalize_generation =
                    Some(self.current_finalize_generation);
            }

            let (immediate, finalize) = rewritten.split_mixed_finalize();
            if let Some(rewritten) = immediate {
                actions.push(CoordinatorAction::ForwardRewritten(rewritten));
            }

            if let Some(mut rewritten) = finalize {
                if self.terminal_finalize_generation == Some(self.current_finalize_generation) {
                    rewritten.apply_finalize_mode(FinalizeMode::NonTerminal);
                    actions.push(CoordinatorAction::ForwardRewritten(rewritten));
                } else if let Some(mut pending) = self.pending_finalize.replace(PendingFinalize {
                    channel,
                    generation: self.current_finalize_generation,
                    response: rewritten,
                }) {
                    pending
                        .response
                        .apply_finalize_mode(FinalizeMode::NonTerminal);
                    actions.push(CoordinatorAction::ForwardRewritten(pending.response));
                }
            }
        } else if let Some(text) = passthrough_text {
            actions.push(CoordinatorAction::ForwardText(text));
        }

        if let Some((code, reason)) = upstream_error {
            actions.push(CoordinatorAction::CloseDownstream {
                code,
                reason: reason.clone(),
            });
            actions.push(CoordinatorAction::ShutdownUpstreams(
                ShutdownSignal::Close { code, reason },
            ));
        }

        actions
    }

    pub(super) fn handle_upstream_closed(
        &mut self,
        channel: usize,
        code: u16,
        reason: String,
    ) -> Vec<CoordinatorAction> {
        self.channels[channel].closed = true;
        let mut actions = Vec::new();

        if code != 1000 {
            self.flush_pending(&mut actions, FinalizeMode::NonTerminal);
            actions.push(CoordinatorAction::CloseDownstream {
                code,
                reason: reason.clone(),
            });
            actions.push(CoordinatorAction::ShutdownUpstreams(
                ShutdownSignal::Close { code, reason },
            ));
            return actions;
        }

        if self.should_release_pending_finalize(channel) {
            if let Some(generation) = self.flush_pending(&mut actions, FinalizeMode::Terminal) {
                self.mark_terminal_finalize_sent(generation);
            }
        }

        if self.channels.iter().all(|channel| channel.closed) {
            actions.push(CoordinatorAction::CloseDownstream {
                code,
                reason: reason.clone(),
            });
            actions.push(CoordinatorAction::ShutdownUpstreams(
                ShutdownSignal::Close { code, reason },
            ));
        }

        actions
    }

    pub(super) fn handle_fatal(&mut self, signal: ShutdownSignal) -> Vec<CoordinatorAction> {
        let mut actions = Vec::new();
        self.flush_pending(&mut actions, FinalizeMode::NonTerminal);
        match signal.clone() {
            ShutdownSignal::Close { code, reason } => {
                actions.push(CoordinatorAction::CloseDownstream {
                    code,
                    reason: reason.clone(),
                });
                actions.push(CoordinatorAction::ShutdownUpstreams(
                    ShutdownSignal::Close { code, reason },
                ));
            }
            ShutdownSignal::Abort => {
                actions.push(CoordinatorAction::AbortDownstream);
                actions.push(CoordinatorAction::ShutdownUpstreams(ShutdownSignal::Abort));
            }
        }
        actions
    }

    pub(super) fn handle_client_closed(
        &mut self,
        code: u16,
        reason: String,
    ) -> Vec<CoordinatorAction> {
        vec![CoordinatorAction::ShutdownUpstreams(
            ShutdownSignal::Close { code, reason },
        )]
    }

    fn flush_pending(
        &mut self,
        actions: &mut Vec<CoordinatorAction>,
        mode: FinalizeMode,
    ) -> Option<u64> {
        if let Some(mut pending) = self.pending_finalize.take() {
            let generation = pending.generation;
            pending.response.apply_finalize_mode(mode);
            actions.push(CoordinatorAction::ForwardRewritten(pending.response));
            return Some(generation);
        }

        None
    }

    fn mark_terminal_finalize_sent(&mut self, generation: u64) {
        self.terminal_finalize_generation = (generation != 0).then_some(generation);
    }

    fn should_release_pending_finalize(&self, channel: usize) -> bool {
        self.pending_finalize.as_ref().is_some_and(|pending| {
            self.terminal_finalize_generation != Some(pending.generation)
                && ((pending.channel == channel
                    && self.other_channel_terminal(channel, pending.generation))
                    || self.channels.iter().all(|channel| channel.closed))
        })
    }

    fn other_channel_terminal(&self, channel: usize, generation: u64) -> bool {
        let other = 1 - channel;
        self.channels[other].closed
            || self.channels[other].last_finalize_generation == Some(generation)
    }
}

#[cfg(test)]
mod tests {
    use super::super::payload::{FinalizeMode, rewrite_split_response};
    use super::*;

    fn rewritten(text: &str) -> RewrittenSplitResponse {
        rewrite_split_response(text, 0, 2, FinalizeMode::Preserve).unwrap()
    }

    fn rewritten_for_channel(text: &str, channel: i32) -> RewrittenSplitResponse {
        rewrite_split_response(text, channel, 2, FinalizeMode::Preserve).unwrap()
    }

    fn assert_non_terminal_finalize(action: CoordinatorAction, transcript: &str) {
        let CoordinatorAction::ForwardRewritten(rewritten) = action else {
            panic!("expected pending finalize flush");
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
        assert_eq!(
            parsed["channel"]["alternatives"][0]["transcript"],
            transcript
        );
    }

    fn assert_close_downstream(action: CoordinatorAction, code: u16, reason: &str) {
        match action {
            CoordinatorAction::CloseDownstream {
                code: actual_code,
                reason: actual_reason,
            } => assert_eq!((actual_code, actual_reason.as_str()), (code, reason)),
            _ => panic!("expected downstream close"),
        }
    }

    fn assert_shutdown_close(action: CoordinatorAction, code: u16, reason: &str) {
        match action {
            CoordinatorAction::ShutdownUpstreams(ShutdownSignal::Close {
                code: actual_code,
                reason: actual_reason,
            }) => assert_eq!((actual_code, actual_reason.as_str()), (code, reason)),
            _ => panic!("expected upstream shutdown close"),
        }
    }

    #[test]
    fn replaces_pending_finalize_by_flushing_older_one_non_terminal() {
        let first = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );
        let second = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"second","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        let first_actions = coordinator.handle_text(0, Some(first), None, None);
        assert!(first_actions.is_empty());

        let second_actions = coordinator.handle_text(0, Some(second), None, None);
        assert_eq!(second_actions.len(), 1);

        let CoordinatorAction::ForwardRewritten(rewritten) =
            second_actions.into_iter().next().unwrap()
        else {
            panic!("expected rewritten flush");
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
    }

    #[test]
    fn keeps_replacing_pending_finalize_until_emitting_channel_closes() {
        let pending = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );
        let later = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"later","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        coordinator.handle_text(0, Some(pending), None, None);
        let close_actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        assert!(
            !close_actions
                .iter()
                .any(|action| matches!(action, CoordinatorAction::ForwardRewritten(_))),
            "pending finalize should stay buffered while the emitting channel is still open"
        );

        let later_actions = coordinator.handle_text(0, Some(later), None, None);
        assert_eq!(later_actions.len(), 1);

        let CoordinatorAction::ForwardRewritten(rewritten) =
            later_actions.into_iter().next().unwrap()
        else {
            panic!("expected rewritten response");
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));

        let terminal_actions = coordinator.handle_upstream_closed(0, 1000, "done".to_string());
        let rewritten = terminal_actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected terminal finalize when the emitting channel closes");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
    }

    #[test]
    fn does_not_release_pending_finalize_until_emitting_channel_closes() {
        let pending = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        assert!(
            coordinator
                .handle_text(0, Some(pending), None, None)
                .is_empty()
        );

        let close_actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        assert!(
            !close_actions
                .iter()
                .any(|action| matches!(action, CoordinatorAction::ForwardRewritten(_))),
            "pending finalize should stay buffered until its own channel closes"
        );
    }

    #[test]
    fn mixed_finalize_batch_forwards_non_final_updates_immediately() {
        let mixed = rewritten(
            r#"[{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"done","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Results","channel_index":[0,1],"duration":0.0,"start":1.0,"is_final":false,"speech_final":false,"from_finalize":false,"channel":{"alternatives":[{"transcript":"live","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}]"#,
        );

        let mut coordinator = SplitCoordinator::default();
        let actions = coordinator.handle_text(0, Some(mixed), None, None);
        assert_eq!(actions.len(), 1);

        let CoordinatorAction::ForwardRewritten(rewritten) = actions.into_iter().next().unwrap()
        else {
            panic!("expected immediate non-final update");
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "live");

        let actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, CoordinatorAction::ForwardRewritten(_)))
        );

        let actions = coordinator.handle_upstream_closed(0, 1000, "done".to_string());
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected buffered finalize to flush on close");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "done");
    }

    #[test]
    fn later_finalize_requests_can_emit_another_terminal_finalize() {
        let first = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );
        let other = rewritten_for_channel(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"other","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
            1,
        );
        let second = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":2.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"second","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        coordinator.handle_finalize_requested();
        assert!(
            coordinator
                .handle_text(0, Some(first), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_text(1, Some(other), None, None);
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected replaced pending finalize to flush");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "first");

        let actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected first finalize cycle to emit terminal finalize");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "other");

        coordinator.handle_finalize_requested();
        assert!(
            coordinator
                .handle_text(0, Some(second), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_upstream_closed(0, 1000, "done".to_string());
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected second finalize cycle to emit terminal finalize");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "second");
    }

    #[test]
    fn later_server_driven_finalize_in_same_generation_stays_buffered() {
        let first = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );
        let second = rewritten_for_channel(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"second","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
            1,
        );
        let later = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":2.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"later","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        assert!(
            coordinator
                .handle_text(0, Some(first), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_text(1, Some(second), None, None);
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected first pending finalize to flush non-terminal");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "first");

        let actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected second finalize to flush terminal");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
        assert_eq!(parsed["channel"]["alternatives"][0]["transcript"], "second");

        let actions = coordinator.handle_text(0, Some(later), None, None);
        assert!(
            actions.is_empty(),
            "later server-driven finalize in the same generation should remain buffered"
        );
        assert!(coordinator.pending_finalize.is_some());
    }

    #[test]
    fn flushes_buffered_finalize_when_sibling_closes_last() {
        let pending = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"last-utterance","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        assert!(
            coordinator
                .handle_text(0, Some(pending), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_upstream_closed(0, 1000, "done".to_string());
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, CoordinatorAction::ForwardRewritten(_))),
            "pending finalize should stay buffered until the sibling also closes"
        );

        let actions = coordinator.handle_upstream_closed(1, 1000, "done".to_string());
        let rewritten = actions
            .into_iter()
            .find_map(|action| match action {
                CoordinatorAction::ForwardRewritten(rewritten) => Some(rewritten),
                _ => None,
            })
            .expect("expected buffered finalize to flush when the sibling closes last");
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten.into_text().unwrap()).unwrap();
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
        assert_eq!(
            parsed["channel"]["alternatives"][0]["transcript"],
            "last-utterance"
        );
    }

    #[test]
    fn non_normal_upstream_close_flushes_pending_and_closes_downstream() {
        let pending = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        assert!(
            coordinator
                .handle_text(0, Some(pending), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_upstream_closed(1, 1011, "speaker_failed".to_string());
        assert_eq!(actions.len(), 3);
        let mut actions = actions.into_iter();

        assert_non_terminal_finalize(actions.next().unwrap(), "first");
        assert_close_downstream(actions.next().unwrap(), 1011, "speaker_failed");
        assert_shutdown_close(actions.next().unwrap(), 1011, "speaker_failed");
    }

    #[test]
    fn fatal_close_flushes_pending_and_preserves_structured_error_for_downstream() {
        let pending = rewritten(
            r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#,
        );

        let mut coordinator = SplitCoordinator::default();
        assert!(
            coordinator
                .handle_text(0, Some(pending), None, None)
                .is_empty()
        );

        let actions = coordinator.handle_fatal(ShutdownSignal::Close {
            code: 1011,
            reason: "upstream_disconnected".to_string(),
        });
        assert_eq!(actions.len(), 3);
        let mut actions = actions.into_iter();

        assert_non_terminal_finalize(actions.next().unwrap(), "first");
        assert_close_downstream(actions.next().unwrap(), 1011, "upstream_disconnected");
        assert_shutdown_close(actions.next().unwrap(), 1011, "upstream_disconnected");
    }

    #[test]
    fn fatal_abort_still_aborts_when_no_structured_close_exists() {
        let actions = SplitCoordinator::default().handle_fatal(ShutdownSignal::Abort);
        assert_eq!(actions.len(), 2);
        let mut actions = actions.into_iter();

        assert!(matches!(
            actions.next().unwrap(),
            CoordinatorAction::AbortDownstream
        ));
        assert!(matches!(
            actions.next().unwrap(),
            CoordinatorAction::ShutdownUpstreams(ShutdownSignal::Abort)
        ));
    }
}
