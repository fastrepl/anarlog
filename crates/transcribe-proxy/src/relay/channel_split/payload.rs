use owhisper_interface::stream::StreamResponse;

fn rewrite_stream_response(response: &mut StreamResponse, channel: i32, total: i32) -> bool {
    match response {
        StreamResponse::TranscriptResponse { from_finalize, .. } => {
            let had_finalize = *from_finalize;
            response.set_channel_index(channel, total);
            had_finalize
        }
        _ => false,
    }
}

fn set_from_finalize(response: &mut StreamResponse, value: bool) {
    if let StreamResponse::TranscriptResponse { from_finalize, .. } = response {
        *from_finalize = value;
    }
}

#[derive(Clone, Copy)]
pub(super) enum FinalizeMode {
    Preserve,
    NonTerminal,
    Terminal,
}

#[allow(clippy::large_enum_variant)]
enum SplitPayload {
    Single(StreamResponse),
    Batch(Vec<StreamResponse>),
}

impl SplitPayload {
    fn parse(text: &str) -> Option<Self> {
        if let Ok(response) = serde_json::from_str::<StreamResponse>(text) {
            return Some(Self::Single(response));
        }

        serde_json::from_str::<Vec<StreamResponse>>(text)
            .ok()
            .map(Self::Batch)
    }

    fn rewrite_channel_index(&mut self, channel: i32, total: i32) -> Vec<usize> {
        let mut finalize_transcripts = Vec::new();

        self.for_each_response_mut(|index, response| {
            if rewrite_stream_response(response, channel, total) {
                finalize_transcripts.push(index);
            }
        });

        finalize_transcripts
    }

    fn force_all_transcripts_non_terminal(&mut self) {
        self.for_each_response_mut(|_, response| set_from_finalize(response, false));
    }

    fn force_last_finalize_terminal(&mut self, finalize_transcripts: &[usize]) {
        self.force_all_transcripts_non_terminal();

        let Some(last_finalize) = finalize_transcripts.last().copied() else {
            return;
        };

        self.for_each_response_mut(|index, response| {
            if index == last_finalize {
                set_from_finalize(response, true);
            }
        });
    }

    fn into_text(self) -> Option<String> {
        match self {
            Self::Single(response) => serde_json::to_string(&response).ok(),
            Self::Batch(responses) => serde_json::to_string(&responses).ok(),
        }
    }

    fn from_vec(mut responses: Vec<StreamResponse>) -> Option<Self> {
        match responses.len() {
            0 => None,
            1 => responses.pop().map(Self::Single),
            _ => Some(Self::Batch(responses)),
        }
    }

    fn split_finalize_transcripts(
        self,
        finalize_transcripts: &[usize],
    ) -> (Option<Self>, Option<Self>) {
        if finalize_transcripts.is_empty() {
            return (Some(self), None);
        }

        let total_responses = match &self {
            Self::Single(_) => 1,
            Self::Batch(responses) => responses.len(),
        };
        let mut finalize_mask = vec![false; total_responses];
        for index in finalize_transcripts {
            if let Some(entry) = finalize_mask.get_mut(*index) {
                *entry = true;
            }
        }

        let responses = match self {
            Self::Single(response) => vec![response],
            Self::Batch(responses) => responses,
        };

        let mut immediate = Vec::new();
        let mut deferred_finalize = Vec::new();
        for (index, response) in responses.into_iter().enumerate() {
            if finalize_mask.get(index).copied().unwrap_or(false) {
                deferred_finalize.push(response);
            } else {
                immediate.push(response);
            }
        }

        (Self::from_vec(immediate), Self::from_vec(deferred_finalize))
    }

    fn for_each_response_mut(&mut self, mut f: impl FnMut(usize, &mut StreamResponse)) {
        match self {
            Self::Single(response) => f(0, response),
            Self::Batch(responses) => {
                for (index, response) in responses.iter_mut().enumerate() {
                    f(index, response);
                }
            }
        }
    }
}

pub(super) struct RewrittenSplitResponse {
    payload: SplitPayload,
    finalize_transcripts: Vec<usize>,
}

impl RewrittenSplitResponse {
    pub(super) fn had_finalize(&self) -> bool {
        !self.finalize_transcripts.is_empty()
    }

    pub(super) fn apply_finalize_mode(&mut self, mode: FinalizeMode) {
        match mode {
            FinalizeMode::Preserve => {}
            FinalizeMode::NonTerminal => self.payload.force_all_transcripts_non_terminal(),
            FinalizeMode::Terminal => self
                .payload
                .force_last_finalize_terminal(&self.finalize_transcripts),
        }
    }

    pub(super) fn split_mixed_finalize(self) -> (Option<Self>, Option<Self>) {
        if !self.had_finalize() {
            return (Some(self), None);
        }

        let has_non_finalize_content = match &self.payload {
            SplitPayload::Single(_) => false,
            SplitPayload::Batch(responses) => responses.len() > self.finalize_transcripts.len(),
        };

        if !has_non_finalize_content {
            return (None, Some(self));
        }

        let (immediate_payload, finalize_payload) = self
            .payload
            .split_finalize_transcripts(&self.finalize_transcripts);

        let immediate = immediate_payload.map(|payload| Self {
            payload,
            finalize_transcripts: Vec::new(),
        });
        let finalize = finalize_payload.map(|payload| {
            let finalize_len = match &payload {
                SplitPayload::Single(_) => 1,
                SplitPayload::Batch(responses) => responses.len(),
            };
            Self {
                payload,
                finalize_transcripts: (0..finalize_len).collect(),
            }
        });

        (immediate, finalize)
    }

    pub(super) fn into_text(self) -> Option<String> {
        self.payload.into_text()
    }
}

pub(super) fn rewrite_split_response(
    text: &str,
    channel: i32,
    total: i32,
    finalize_mode: FinalizeMode,
) -> Option<RewrittenSplitResponse> {
    let mut payload = SplitPayload::parse(text)?;
    let finalize_transcripts = payload.rewrite_channel_index(channel, total);
    let mut rewritten = RewrittenSplitResponse {
        payload,
        finalize_transcripts,
    };
    rewritten.apply_finalize_mode(finalize_mode);
    Some(rewritten)
}

#[cfg(test)]
mod tests {
    use owhisper_interface::stream::{Alternatives, Channel, Metadata};

    use super::*;

    fn transcript_response(from_finalize: bool) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: 0.0,
            is_final: from_finalize,
            speech_final: from_finalize,
            from_finalize,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: "hello".to_string(),
                    words: vec![],
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }

    #[test]
    fn rewrite_split_response_results() {
        let input = r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":false,"speech_final":false,"from_finalize":false,"channel":{"alternatives":[{"transcript":"","confidence":0.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#;
        let result = rewrite_split_response(input, 1, 2, FinalizeMode::NonTerminal).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.into_text().unwrap()).unwrap();
        assert_eq!(parsed["channel_index"], serde_json::json!([1, 2]));
        assert_eq!(parsed["from_finalize"], serde_json::json!(false));
        assert!(!parsed["from_finalize"].as_bool().unwrap());
    }

    #[test]
    fn rewrite_split_response_non_results() {
        let input =
            r#"{"type":"Metadata","request_id":"abc","created":"","duration":0.0,"channels":1}"#;
        let result = rewrite_split_response(input, 1, 2, FinalizeMode::NonTerminal).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.into_text().unwrap()).unwrap();
        assert!(parsed.get("channel_index").is_none());
    }

    #[test]
    fn rewrite_split_response_array_terminalizes_only_last_finalize() {
        let input = r#"[{"type":"Results","channel_index":[],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Results","channel_index":[],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"second","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Metadata","request_id":"abc","created":"","duration":0.0,"channels":1}]"#;
        let result = rewrite_split_response(input, 0, 2, FinalizeMode::Terminal).unwrap();
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&result.into_text().unwrap()).unwrap();
        assert_eq!(parsed[0]["channel_index"], serde_json::json!([0, 2]));
        assert_eq!(parsed[1]["channel_index"], serde_json::json!([0, 2]));
        assert_eq!(parsed[0]["from_finalize"], serde_json::json!(false));
        assert_eq!(parsed[1]["from_finalize"], serde_json::json!(true));
        assert!(parsed[2].get("channel_index").is_none());
    }

    #[test]
    fn rewrite_split_response_invalid_json() {
        assert!(rewrite_split_response("not json", 0, 2, FinalizeMode::Preserve).is_none());
    }

    #[test]
    fn rewrite_stream_response_transcript_sets_channel_and_reports_finalize() {
        let mut response = transcript_response(true);

        let had_finalize = rewrite_stream_response(&mut response, 1, 2);

        assert!(had_finalize);
        match response {
            StreamResponse::TranscriptResponse { channel_index, .. } => {
                assert_eq!(channel_index, vec![1, 2]);
            }
            _ => panic!("expected transcript response"),
        }
    }

    #[test]
    fn rewrite_stream_response_non_transcript_is_noop() {
        let mut response = StreamResponse::TerminalResponse {
            request_id: "abc".to_string(),
            created: "".to_string(),
            duration: 0.0,
            channels: 1,
        };

        let had_finalize = rewrite_stream_response(&mut response, 1, 2);

        assert!(!had_finalize);
        match response {
            StreamResponse::TerminalResponse { request_id, .. } => {
                assert_eq!(request_id, "abc");
            }
            _ => panic!("expected terminal response"),
        }
    }

    #[test]
    fn set_from_finalize_only_changes_transcript_responses() {
        let mut transcript = transcript_response(false);
        set_from_finalize(&mut transcript, true);
        match transcript {
            StreamResponse::TranscriptResponse { from_finalize, .. } => {
                assert!(from_finalize);
            }
            _ => panic!("expected transcript response"),
        }

        let mut terminal = StreamResponse::TerminalResponse {
            request_id: "abc".to_string(),
            created: "".to_string(),
            duration: 0.0,
            channels: 1,
        };
        set_from_finalize(&mut terminal, true);
        match terminal {
            StreamResponse::TerminalResponse { request_id, .. } => {
                assert_eq!(request_id, "abc");
            }
            _ => panic!("expected terminal response"),
        }
    }

    #[test]
    fn rewrite_split_response_preserves_finalize_when_not_forced() {
        let input = r#"{"type":"Results","channel_index":[0,1],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"done","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}"#;
        let result = rewrite_split_response(input, 1, 2, FinalizeMode::Preserve).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.into_text().unwrap()).unwrap();

        assert_eq!(parsed["channel_index"], serde_json::json!([1, 2]));
        assert_eq!(parsed["from_finalize"], serde_json::json!(true));
    }

    #[test]
    fn rewrite_split_response_array_force_non_terminal_clears_all_transcripts() {
        let input = r#"[{"type":"Results","channel_index":[],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"first","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Results","channel_index":[],"duration":0.0,"start":1.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"second","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}}]"#;
        let result = rewrite_split_response(input, 0, 2, FinalizeMode::NonTerminal).unwrap();
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&result.into_text().unwrap()).unwrap();

        assert_eq!(parsed[0]["from_finalize"], serde_json::json!(false));
        assert_eq!(parsed[1]["from_finalize"], serde_json::json!(false));
    }

    #[test]
    fn split_mixed_finalize_separates_non_final_updates() {
        let input = r#"[{"type":"Results","channel_index":[],"duration":0.0,"start":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel":{"alternatives":[{"transcript":"done","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Results","channel_index":[],"duration":0.0,"start":1.0,"is_final":false,"speech_final":false,"from_finalize":false,"channel":{"alternatives":[{"transcript":"live","confidence":1.0,"words":[]}]},"metadata":{"request_id":"","model_info":{"name":"","version":"","arch":""},"model_uuid":""}},{"type":"Metadata","request_id":"abc","created":"","duration":0.0,"channels":1}]"#;
        let rewritten = rewrite_split_response(input, 0, 2, FinalizeMode::Preserve).unwrap();

        let (immediate, finalize) = rewritten.split_mixed_finalize();

        let immediate: Vec<serde_json::Value> =
            serde_json::from_str(&immediate.unwrap().into_text().unwrap()).unwrap();
        assert_eq!(immediate.len(), 2);
        assert_eq!(immediate[0]["from_finalize"], serde_json::json!(false));
        assert_eq!(
            immediate[0]["channel"]["alternatives"][0]["transcript"],
            "live"
        );
        assert_eq!(immediate[1]["type"], "Metadata");

        let finalize: serde_json::Value =
            serde_json::from_str(&finalize.unwrap().into_text().unwrap()).unwrap();
        assert_eq!(finalize["from_finalize"], serde_json::json!(true));
        assert_eq!(finalize["channel"]["alternatives"][0]["transcript"], "done");
    }
}
