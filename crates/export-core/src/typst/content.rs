use crate::ExportInput;

use super::markdown::markdown_to_typst;
use super::utils::{escape_typst_literal, escape_typst_string};

// Palette mirrors apps/web/BRAND.md (warm stone scale + brand yellow).
fn build_preamble() -> String {
    r##"
#let ink = rgb("#332d23")
#let muted = rgb("#57534e")
#let subtle = rgb("#b7b6b5")
#let hairline = rgb("#e5e5e3")
#let surface = rgb("#f2f0ed")
#let brand-yellow = rgb("#ffedbb")
#let bright = rgb("#8a7d69")

#set page(
  paper: "a4",
  margin: (top: 2.4cm, bottom: 2.4cm, left: 2.2cm, right: 2.2cm),
  header: context [
    #grid(
      columns: (1fr, auto),
      text(size: 8pt, fill: muted)[#session-title],
      text(size: 8pt, fill: subtle, tracking: 0.14em)[ANARLOG],
    )
    #v(5pt)
    #line(length: 100%, stroke: 0.5pt + hairline)
  ],
  footer: context [
    #align(center, text(size: 8pt, fill: subtle)[
      #counter(page).display("1 / 1", both: true)
    ])
  ],
)

#set text(
  font: "Pretendard",
  size: 10.5pt,
  fill: ink,
  lang: "en",
)

#set par(
  justify: true,
  leading: 0.75em,
  spacing: 1.2em,
)

#set list(
  marker: ([•], [–], [·]),
  indent: 6pt,
  body-indent: 0.55em,
  spacing: 0.55em,
)

#set enum(
  indent: 6pt,
  body-indent: 0.55em,
  spacing: 0.55em,
)

#show heading.where(level: 1): it => block(
  width: 100%,
  above: 0pt,
  below: 1.2em,
  stroke: (bottom: 0.5pt + hairline),
  inset: (bottom: 8pt),
)[
  #text(font: "Libertinus Serif", size: 20pt, weight: "bold", fill: ink, it.body)
]

#show heading.where(level: 2): it => block(
  above: 1.5em,
  below: 0.7em,
  text(size: 13.5pt, weight: "bold", fill: ink, it.body)
)

#show heading.where(level: 3): it => block(
  above: 1.3em,
  below: 0.6em,
  text(size: 11.5pt, weight: "semibold", fill: ink, it.body)
)

#show heading.where(level: 4): it => block(
  above: 1.2em,
  below: 0.5em,
  text(size: 10.5pt, weight: "semibold", fill: muted, it.body)
)

#show heading.where(level: 5): it => block(
  above: 1.2em,
  below: 0.5em,
  text(size: 10pt, weight: "semibold", fill: muted, it.body)
)

#show link: it => underline(
  offset: 2.5pt,
  stroke: (paint: subtle, thickness: 0.4pt),
  text(fill: ink, it),
)

#show quote.where(block: true): it => block(
  width: 100%,
  fill: surface,
  stroke: (left: 2pt + bright),
  inset: (left: 12pt, right: 12pt, top: 8pt, bottom: 8pt),
  it.body,
)

#show raw.where(block: false): it => box(
  fill: surface,
  radius: 3pt,
  inset: (x: 4pt),
  outset: (y: 2.5pt),
  text(font: "DejaVu Sans Mono", size: 8.5pt, fill: ink, it),
)

#show raw.where(block: true): it => block(
  width: 100%,
  fill: surface,
  radius: 6pt,
  inset: (x: 12pt, y: 10pt),
  text(font: "DejaVu Sans Mono", size: 8.5pt, fill: ink, it),
)

"##
    .to_string()
}

fn cover_meta_label(value: &str) -> String {
    format!(
        "  #text(size: 7.5pt, weight: \"bold\", fill: subtle, tracking: 0.14em)[{}]",
        escape_typst_string(value)
    )
}

fn build_cover_page(
    title: &str,
    created_at: &str,
    participants: &[String],
    event_title: Option<&str>,
    duration: Option<&str>,
) -> String {
    let mut cover = String::new();

    cover.push_str("#page(margin: 0pt, header: none, footer: none, fill: white)[\n");
    // Warm wash + dotted grid over the top of the cover.
    cover.push_str(
        "  #place(top + left, rect(width: 100%, height: 34%, fill: gradient.linear(brand-yellow, white, dir: ttb)))\n",
    );
    cover.push_str(
        "  #place(top + left, rect(width: 100%, height: 34%, fill: pattern(size: (6mm, 6mm))[#place(center + horizon, circle(radius: 0.35mm, fill: rgb(\"#e0d6bf\")))]))\n",
    );

    cover.push_str("  #pad(x: 22mm)[\n");
    cover.push_str("    #v(24mm)\n");
    cover.push_str("    #image(\"anarlog-logo.svg\", width: 24mm)\n");
    cover.push_str("    #v(22mm)\n");

    cover.push_str(
        "    #text(size: 9pt, weight: \"bold\", fill: muted, tracking: 0.2em)[SESSION NOTES]\n",
    );
    cover.push_str("    #v(3mm)\n");
    cover.push_str("    #line(length: 16mm, stroke: 1.2pt + brand-yellow)\n");
    cover.push_str("    #v(9mm)\n");

    let escaped_title = escape_typst_string(title);
    cover.push_str(&format!(
        "    #text(font: \"Libertinus Serif\", size: 33pt, weight: \"bold\", fill: ink)[{}]\n",
        escaped_title
    ));
    cover.push_str("    #v(8mm)\n");

    let escaped_date = escape_typst_string(created_at);
    match duration {
        Some(dur) if !dur.is_empty() => {
            let escaped_duration = escape_typst_string(dur);
            cover.push_str(&format!(
                "    #text(size: 11pt, fill: muted)[{} #sym.dot.c {}]\n",
                escaped_date, escaped_duration
            ));
        }
        _ => {
            cover.push_str(&format!(
                "    #text(size: 11pt, fill: muted)[{}]\n",
                escaped_date
            ));
        }
    }
    cover.push_str("    #v(12mm)\n");

    if let Some(event) = event_title {
        let escaped_event = escape_typst_string(event);
        cover.push_str(
            "    #block(width: 100%, fill: white, stroke: 0.5pt + hairline, radius: 8pt, inset: (x: 16pt, y: 12pt))[\n",
        );
        cover.push_str(&cover_meta_label("MEETING"));
        cover.push('\n');
        cover.push_str("      #v(3pt)\n");
        cover.push_str(&format!(
            "      #text(size: 12pt, weight: \"semibold\", fill: ink)[{}]\n",
            escaped_event
        ));
        cover.push_str("    ]\n");
        cover.push_str("    #v(4mm)\n");
    }

    if !participants.is_empty() {
        let joined = participants
            .iter()
            .map(|p| escape_typst_string(p))
            .collect::<Vec<_>>()
            .join(", ");
        cover.push_str(
            "    #block(width: 100%, fill: white, stroke: 0.5pt + hairline, radius: 8pt, inset: (x: 16pt, y: 12pt))[\n",
        );
        cover.push_str(&cover_meta_label("PARTICIPANTS"));
        cover.push('\n');
        cover.push_str("      #v(3pt)\n");
        cover.push_str(&format!("      #text(size: 11pt, fill: ink)[{}]\n", joined));
        cover.push_str("    ]\n");
    }

    cover.push_str("    #v(1fr)\n");
    cover.push_str("    #line(length: 100%, stroke: 0.5pt + hairline)\n");
    cover.push_str("    #v(4mm)\n");
    cover.push_str("    #table(columns: (1fr, auto), stroke: none, inset: 0pt,\n");
    cover.push_str("      [#image(\"anarlog-logo.svg\", width: 13mm)],\n");
    cover.push_str(
        "      [#align(right, text(size: 9pt, fill: subtle)[#link(\"https://anarlog.so\")[anarlog.so]])],\n",
    );
    cover.push_str("    )\n");
    cover.push_str("    #v(14mm)\n");

    cover.push_str("  ]\n");
    cover.push_str("]\n\n");

    cover
}

fn build_transcript_item(speaker: &str, text: &str) -> String {
    let escaped_speaker = escape_typst_string(speaker);
    let escaped_text = escape_typst_string(text);
    format!(
        concat!(
            "#block(width: 100%, stroke: (bottom: 0.5pt + hairline), inset: (bottom: 7pt))[\n",
            "  #grid(columns: (30mm, 1fr), column-gutter: 10pt,\n",
            "    [#text(size: 8.5pt, weight: \"bold\", fill: muted, tracking: 0.05em)[#upper[{}]]],\n",
            "    [#text(size: 10.5pt, fill: ink)[{}]],\n",
            "  )\n",
            "]\n",
            "#v(7pt)\n"
        ),
        escaped_speaker, escaped_text
    )
}

pub fn build_typst_content(input: &ExportInput) -> String {
    let mut content = String::new();

    let session_title = input
        .metadata
        .as_ref()
        .map(|m| m.title.replace(['\n', '\r'], " "))
        .unwrap_or_default();
    content.push_str(&format!(
        "#let session-title = \"{}\"\n",
        escape_typst_literal(&session_title)
    ));

    content.push_str(&build_preamble());

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

    if let Some(memo_md) = &input.memo_md {
        let memo_content = markdown_to_typst(memo_md);
        if !memo_content.trim().is_empty() {
            content.push_str("= Memo\n\n");
            content.push_str(&memo_content);
            content.push_str("\n\n");
        }
    }

    let typst_content = markdown_to_typst(&input.enhanced_md);
    if !typst_content.trim().is_empty() {
        if input.memo_md.as_ref().is_some_and(|m| !m.trim().is_empty()) {
            content.push_str("\n#pagebreak()\n\n");
        }
        content.push_str("= Summary\n\n");
        content.push_str(&typst_content);
    }

    if let Some(transcript) = &input.transcript
        && !transcript.items.is_empty()
    {
        if input.metadata.is_some()
            || !input.enhanced_md.trim().is_empty()
            || input.memo_md.as_ref().is_some_and(|m| !m.trim().is_empty())
        {
            content.push_str("\n#pagebreak()\n\n");
        }
        content.push_str("= Transcript\n\n");

        for item in &transcript.items {
            let speaker = item.speaker.as_deref().unwrap_or("Unknown");
            content.push_str(&build_transcript_item(speaker, &item.text));
        }
    }

    content
}
