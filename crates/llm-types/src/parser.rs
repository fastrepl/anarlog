use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_until},
    character::complete::multispace0,
    combinator::map,
    sequence::delimited,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    TextDelta(String),
    Reasoning(String),
    ToolCall {
        name: String,
        arguments: HashMap<String, serde_json::Value>,
    },
}

pub struct StreamingParser {
    buffer: String,
}

impl Default for StreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn flush(&mut self) -> Option<Response> {
        if self.buffer.is_empty() {
            return None;
        }
        Some(Response::TextDelta(std::mem::take(&mut self.buffer)))
    }

    pub fn process_chunk(&mut self, chunk: &str) -> Vec<Response> {
        self.buffer.push_str(chunk);
        let mut responses = Vec::new();

        while let Some(response) = self.try_parse_next() {
            responses.push(response);
        }

        if !self.buffer.is_empty() && !self.looks_like_block_start() {
            let text = self.buffer.clone();
            self.buffer.clear();
            responses.push(Response::TextDelta(text));
        }

        responses
    }

    fn try_parse_next(&mut self) -> Option<Response> {
        if let Ok((remaining, content)) = parse_think_block(&self.buffer) {
            self.buffer = remaining.to_string();
            return Some(Response::Reasoning(content));
        }

        if let Ok((remaining, (name, arguments))) = parse_tool_call_block(&self.buffer) {
            let arguments: HashMap<String, serde_json::Value> =
                serde_json::from_str(&arguments).unwrap_or_default();

            self.buffer = remaining.to_string();
            return Some(Response::ToolCall { name, arguments });
        }

        if let Some(pos) = self.find_next_block_start()
            && pos > 0
        {
            let text = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos..].to_string();
            return Some(Response::TextDelta(text));
        }

        None
    }

    fn looks_like_block_start(&self) -> bool {
        self.buffer.trim_start().starts_with("<think>")
            || self.buffer.trim_start().starts_with("<tool_call>")
    }

    fn find_next_block_start(&self) -> Option<usize> {
        let think_pos = self.buffer.find("<think>");
        let tool_pos = self.buffer.find("<tool_call>");

        match (think_pos, tool_pos) {
            (Some(t), Some(tc)) => Some(t.min(tc)),
            (Some(t), None) => Some(t),
            (None, Some(tc)) => Some(tc),
            (None, None) => None,
        }
    }
}

fn parse_think_block(input: &str) -> IResult<&str, String> {
    let mut parser = map(
        delimited(tag("<think>"), take_until("</think>"), tag("</think>")),
        |content: &str| content.trim().to_string(),
    );
    parser.parse(input)
}

fn parse_tool_call_block(input: &str) -> IResult<&str, (String, String)> {
    let (input, _) = tag("<tool_call>")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, json_content) = take_until("</tool_call>")(input)?;
    let (input, _) = tag("</tool_call>")(input)?;
    let (input, _) = multispace0(input)?;

    let json_trimmed = json_content.trim();
    let parsed: serde_json::Value = serde_json::from_str(json_trimmed)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))?;

    let name = parsed["name"]
        .as_str()
        .ok_or_else(|| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))?
        .to_string();

    let arguments = parsed["arguments"].to_string();

    Ok((input, (name, arguments)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text() {
        let mut parser = StreamingParser::new();

        let items = {
            let mut items = vec![];
            items.extend(parser.process_chunk("Hello, "));
            items.extend(parser.process_chunk("world!"));
            items
        };

        assert_eq!(
            items,
            vec![
                Response::TextDelta("Hello, ".to_string()),
                Response::TextDelta("world!".to_string())
            ]
        );
    }

    #[test]
    fn test_thinking_block() {
        let mut parser = StreamingParser::new();

        let items = {
            let mut items = vec![];
            items.extend(parser.process_chunk("<think>\nI need to "));
            items.extend(parser.process_chunk("process this request.\n"));
            items.extend(parser.process_chunk("</think>\nHere's my "));
            items.extend(parser.process_chunk("response."));
            items
        };

        assert_eq!(
            items,
            [
                Response::Reasoning("I need to process this request.".to_string()),
                Response::TextDelta("\nHere's my ".to_string()),
                Response::TextDelta("response.".to_string())
            ]
        );
    }

    #[test]
    fn test_simple_tool_call() {
        let mut parser = StreamingParser::new();

        let items = {
            let mut items = vec![];
            items.extend(parser.process_chunk("<tool_call>\n"));
            items.extend(parser.process_chunk(r#"{"name": "greet", "#));
            items.extend(parser.process_chunk(r#""arguments": {"text": "#));
            items.extend(parser.process_chunk(r#""Hello!"}}"#));
            items.extend(parser.process_chunk("\n</tool_call>"));
            items
        };

        assert_eq!(
            items,
            vec![Response::ToolCall {
                name: "greet".to_string(),
                arguments: HashMap::from([(
                    "text".to_string(),
                    serde_json::Value::String("Hello!".to_string())
                )])
            }]
        );
    }

    #[test]
    fn test_tool_call_after_thinking() {
        let mut parser = StreamingParser::new();

        let items = {
            let mut items = vec![];
            items.extend(parser.process_chunk("<think>\nI need to "));
            items.extend(parser.process_chunk("process this request.\n"));
            items.extend(parser.process_chunk("</think>\n"));
            items.extend(parser.process_chunk("<tool_call>\n"));
            items.extend(parser.process_chunk(r#"{"name": "greet", "#));
            items.extend(parser.process_chunk(r#""arguments": {"text": "#));
            items.extend(parser.process_chunk(r#""Hello!"}}"#));
            items.extend(parser.process_chunk("\n</tool_call>"));
            items
        };

        assert_eq!(
            items,
            [
                Response::Reasoning("I need to process this request.".to_string()),
                Response::TextDelta("\n".to_string()),
                Response::ToolCall {
                    name: "greet".to_string(),
                    arguments: HashMap::from([(
                        "text".to_string(),
                        serde_json::Value::String("Hello!".to_string())
                    )])
                }
            ]
        );
    }

    #[test]
    fn test_summary() {
        let reasoning = r###"
<think>
something blabla1
something blabla2
</think>        
     "###
        .trim();

        let summary = r###"
# Char Overview

Char is an AI-powered notepad designed for private meetings with complete on-device processing. No data leaves your computer, with optional telemetry.

# How It Works

*   Create a new note and press the record button
*   Live transcript appears in the right panel as people speak
*   Jot down key points during the meeting
*   Stop recording to generate an AI summary that blends your notes with the transcript
*   Processing happens locally using an on-device AI model

# Key Features

*   **100% Private**: No wifi, cloud, or third-party services required
*   **Live Transcription**: Real-time speech-to-text during meetings
*   **AI Summaries**: Automatic synthesis of notes and transcripts
*   **Additional Tools**: AI chat, meeting templates, customizable workflow settings

# Getting Started

*   Documentation available for detailed guidance
*   Discord community for support and feedback
        "###.trim();

        fn build_chunks(text: &str) -> Vec<String> {
            use rand::Rng;
            let mut rng = rand::rng();

            let mut chunks = Vec::new();

            let chars: Vec<char> = text.chars().collect();
            let mut pos = 0;
            while pos < chars.len() {
                let chunk_size = rng.random_range(3..8);
                let end_pos = std::cmp::min(pos + chunk_size, chars.len());
                let chunk: String = chars[pos..end_pos].iter().collect();
                chunks.push(chunk);
                pos = end_pos;
            }
            chunks
        }

        {
            let chunks = build_chunks(summary);
            let mut parser = StreamingParser::new();

            let items = {
                let mut items = vec![];
                for chunk in chunks {
                    items.extend(parser.process_chunk(&chunk));
                }
                items
            };

            let restored = items
                .iter()
                .map(|item| match item {
                    Response::TextDelta(text) => text.clone(),
                    _ => "".to_string(),
                })
                .collect::<String>();

            assert_eq!(restored, summary);
        }

        {
            let text = format!("{}\n{}", reasoning, summary);

            let chunks = build_chunks(&text);
            let mut parser = StreamingParser::new();

            let items = {
                let mut items = vec![];
                for chunk in chunks {
                    items.extend(parser.process_chunk(&chunk));
                }
                items
            };

            let restored = items
                .iter()
                .map(|item| match item {
                    Response::TextDelta(text) => text.clone(),
                    Response::Reasoning(reasoning) => {
                        format!("<think>\n{}\n</think>", reasoning)
                    }
                    _ => "".to_string(),
                })
                .collect::<String>();

            assert_eq!(restored, text);
        }
    }
}
