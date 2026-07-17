use crate::proto::{Note, ParagraphStyle};

#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub link: Option<String>,
    pub style_type: Option<i32>,
    pub paragraph_style: Option<ParagraphStyle>,
}

impl TextSpan {
    pub fn new(text: String) -> Self {
        Self {
            text,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            link: None,
            style_type: None,
            paragraph_style: None,
        }
    }
}

pub fn extract_text_spans(note: &Note) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    let mut current_char_index = 0;

    let chars: Vec<char> = note.note_text.chars().collect();

    for attr_run in &note.attribute_run {
        if attr_run.attachment_info.is_some() {
            current_char_index += attr_run.length as usize;
            continue;
        }

        let length = attr_run.length as usize;
        let end_char_index = current_char_index + length;

        if end_char_index > chars.len() {
            break;
        }

        let text_slice: String = chars[current_char_index..end_char_index].iter().collect();

        let mut span = TextSpan::new(text_slice.to_string());

        if let Some(font_weight) = attr_run.font_weight {
            match font_weight {
                1 => span.bold = true,
                2 => span.italic = true,
                3 => {
                    span.bold = true;
                    span.italic = true;
                }
                _ => {}
            }
        }

        if let Some(underlined) = attr_run.underlined {
            span.underline = underlined == 1;
        }

        if let Some(strikethrough) = attr_run.strikethrough {
            span.strikethrough = strikethrough == 1;
        }

        if let Some(ref link) = attr_run.link
            && !link.is_empty()
        {
            span.link = Some(link.clone());
        }

        if let Some(ref para_style) = attr_run.paragraph_style {
            span.style_type = para_style.style_type;
            span.paragraph_style = Some(para_style.clone());
        }

        spans.push(span);
        current_char_index = end_char_index;
    }

    spans
}

pub fn extract_plaintext(note: &Note) -> String {
    note.note_text.clone()
}
