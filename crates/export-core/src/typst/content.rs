use crate::ExportInput;

use super::markdown::markdown_to_typst;
use super::utils::escape_typst_string;

fn build_preamble() -> String {
    r##"
#let accent-color = rgb("#2563eb")
#let muted-color = rgb("#6b7280")
#let light-bg = rgb("#f8fafc")

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 2.5cm, left: 2.5cm, right: 2.5cm),
)

#set text(
  font: "Pretendard",
  size: 11pt,
  lang: "en",
)

#set par(
  justify: true,
  leading: 0.65em,
)

#show heading.where(level: 1): it => block(
  above: 1.5em,
  below: 1em,
  text(size: 18pt, weight: "bold", it.body)
)

#show heading.where(level: 2): it => block(
  above: 1.3em,
  below: 0.8em,
  text(size: 14pt, weight: "bold", it.body)
)

#show heading.where(level: 3): it => block(
  above: 1.2em,
  below: 0.6em,
  text(size: 12pt, weight: "bold", it.body)
)

#show link: it => text(fill: accent-color, it)

#show quote: it => block(
  inset: (left: 1em, right: 1em, top: 0.5em, bottom: 0.5em),
  stroke: (left: 2pt + rgb("#d1d5db")),
  fill: rgb("#f9fafb"),
  it.body
)

"##
    .to_string()
}

fn build_cover_page(
    title: &str,
    created_at: &str,
    participants: &[String],
    event_title: Option<&str>,
    duration: Option<&str>,
) -> String {
    let mut cover = String::new();

    cover.push_str("#page(margin: (top: 4cm, bottom: 2.5cm, left: 2.5cm, right: 2.5cm))[\n");
    cover.push_str("  #align(center)[\n");

    cover.push_str("    #v(2cm)\n");

    let escaped_title = escape_typst_string(title);
    cover.push_str(&format!(
        "    #text(size: 28pt, weight: \"bold\")[{}]\n",
        escaped_title
    ));

    cover.push_str("    #v(1.5em)\n");

    let escaped_date = escape_typst_string(created_at);
    cover.push_str(&format!(
        "    #text(size: 12pt, fill: muted-color)[{}]\n",
        escaped_date
    ));

    if let Some(dur) = duration {
        let escaped_duration = escape_typst_string(dur);
        cover.push_str(&format!(
            "    #text(size: 11pt, fill: muted-color)[ #sym.dot.c {}]\n",
            escaped_duration
        ));
    }

    cover.push_str("    #v(2em)\n");

    if let Some(event) = event_title {
        let escaped_event = escape_typst_string(event);
        cover.push_str(&format!(
            "    #block(fill: light-bg, inset: 12pt, radius: 6pt, width: 80%)[\n      #text(size: 11pt, fill: muted-color)[Meeting:] #text(size: 11pt)[{}]\n    ]\n",
            escaped_event
        ));
        cover.push_str("    #v(1em)\n");
    }

    if !participants.is_empty() {
        cover.push_str("    #block(fill: light-bg, inset: 12pt, radius: 6pt, width: 80%)[\n");
        cover.push_str(
            "      #text(size: 11pt, fill: muted-color)[Participants:]\n      #v(0.5em)\n",
        );
        for participant in participants {
            let escaped_participant = escape_typst_string(participant);
            cover.push_str(&format!(
                "      #text(size: 11pt)[#sym.bullet {}]\n      #v(0.3em)\n",
                escaped_participant
            ));
        }
        cover.push_str("    ]\n");
    }

    cover.push_str("    #v(1fr)\n");

    cover.push_str("    #text(size: 10pt, fill: muted-color)[Exported from Char]\n");

    cover.push_str("  ]\n");
    cover.push_str("]\n\n");

    cover
}

pub fn build_typst_content(input: &ExportInput) -> String {
    let mut content = build_preamble();

    if let Some(metadata) = &input.metadata {
        let cover = build_cover_page(
            &metadata.title,
            &metadata.created_at,
            &metadata.participants,
            metadata.event_title.as_deref(),
            metadata.duration.as_deref(),
        );
        content.push_str(&cover);
    }

    let typst_content = markdown_to_typst(&input.enhanced_md);
    if !typst_content.trim().is_empty() {
        content.push_str(&typst_content);
    }

    if let Some(transcript) = &input.transcript
        && !transcript.items.is_empty()
    {
        if input.metadata.is_some() || !input.enhanced_md.trim().is_empty() {
            content.push_str("\n#pagebreak()\n\n");
        }
        content.push_str("= Transcript\n\n");

        for item in &transcript.items {
            let speaker = item.speaker.as_deref().unwrap_or("Unknown");
            let escaped_speaker = escape_typst_string(speaker);
            let escaped_text = escape_typst_string(&item.text);
            content.push_str(&format!(
                "#block(fill: light-bg, inset: 10pt, radius: 4pt, width: 100%, spacing: 0.8em)[#text(weight: \"semibold\", fill: accent-color)[{}:] {}]\n",
                escaped_speaker, escaped_text
            ));
        }
    }

    content
}
