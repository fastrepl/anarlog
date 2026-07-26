#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse markdown")]
    MarkdownParseError(String),
    #[error("failed to render markdown")]
    MarkdownRenderError(String),
    #[error("failed to render html")]
    HTMLRenderError(String),
    #[error("failed to parse html")]
    HTMLParseError(String),
}

type TextTransformation = Box<dyn Fn(&mut String)>;
type MdTransformation = Box<dyn Fn(&mut markdown::mdast::Node)>;

pub fn opinionated_md_to_html(text: impl AsRef<str>) -> Result<String, Error> {
    let md = md_to_md(text)?;
    let md_with_mentions = transform_mentions_in_markdown(&md);
    md_to_html(&md_with_mentions)
}

pub fn opinionated_md_to_md(text: impl AsRef<str>) -> Result<String, Error> {
    md_to_md(text)
}

fn md_to_md(text: impl AsRef<str>) -> Result<String, Error> {
    let mut text = text.as_ref().to_string();

    let txt_transformations: Vec<TextTransformation> = vec![Box::new(remove_char_repeat)];

    for t in txt_transformations {
        t(&mut text);
    }

    let mut ast = markdown::to_mdast(text.as_ref(), &markdown::ParseOptions::default())
        .map_err(|e| Error::MarkdownParseError(e.to_string()))?;

    let md_transformations: Vec<MdTransformation> = vec![
        Box::new(remove_thematic_break),
        Box::new(remove_empty_headings),
        Box::new(|node| {
            set_heading_level_from(node, 1, false);
        }),
        Box::new(flatten_headings),
        Box::new(convert_ordered_to_unordered),
        Box::new(add_paragraphs_before_headings),
    ];

    for t in md_transformations {
        t(&mut ast);
    }

    let md = mdast_util_to_markdown::to_markdown_with_options(
        &ast,
        &mdast_util_to_markdown::Options {
            bullet: '-',
            ..Default::default()
        },
    )
    .map_err(|e| Error::MarkdownRenderError(e.to_string()))?;

    Ok(md)
}

fn md_to_html(text: &str) -> Result<String, Error> {
    let html = markdown::to_html_with_options(
        text,
        &markdown::Options {
            parse: markdown::ParseOptions {
                constructs: markdown::Constructs {
                    gfm_autolink_literal: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                ..Default::default()
            },
        },
    )
    .map_err(|e| Error::HTMLRenderError(e.to_string()))?;

    let dom = tl::parse(&html, tl::ParserOptions::default())
        .map_err(|e| Error::HTMLParseError(e.to_string()))?;

    Ok(dom.outer_html())
}

fn transform_mentions_in_markdown(markdown: &str) -> String {
    // @[label](type:id)
    let re = regex::Regex::new(r"@\[([^\]]+)\]\(([^:]+):([^)]+)\)").unwrap();

    re.replace_all(markdown, |caps: &regex::Captures| {
        let label = &caps[1];
        let mention_type = &caps[2];
        let id = &caps[3];

        let app_url = format!("/app/{}/{}", mention_type, id);

        format!(
            r#"<a class="mention" data-mention="true" data-id="{}" data-type="{}" data-label="{}" href="javascript:void(0)" onclick="event.preventDefault(); if (window.__HYPR_NAVIGATE__) window.__HYPR_NAVIGATE__('{}');">@{}</a>"#,
            id, mention_type, label, app_url, label
        )
    }).to_string()
}

fn remove_char_repeat(text: &mut String) {
    let lines: Vec<&str> = text.lines().collect();
    let filtered_lines: Vec<String> = lines
        .iter()
        .filter_map(|line| {
            if line.len() >= 6 {
                let chars: Vec<char> = line.chars().collect();
                if !chars.is_empty() {
                    let first_char = chars[0];

                    if !first_char.is_alphanumeric()
                        && !first_char.is_whitespace()
                        && chars.iter().all(|&c| c == first_char)
                    {
                        return None;
                    }
                }
            }
            Some(line.to_string())
        })
        .collect();

    *text = filtered_lines.join("\n");
}

fn convert_ordered_to_unordered(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::List(list) = node {
        list.ordered = false;
        list.spread = false;
    }

    if let Some(children) = node.children_mut() {
        for child in children {
            convert_ordered_to_unordered(child);
        }
    }
}

fn set_heading_level_from(node: &mut markdown::mdast::Node, depth: u8, header_found: bool) -> bool {
    let mut found_any_heading = header_found;

    if let markdown::mdast::Node::Heading(heading) = node {
        found_any_heading = true;
        heading.depth = depth;

        if let Some(children) = node.children_mut() {
            for child in children {
                set_heading_level_from(child, depth + 1, found_any_heading);
            }
        }
    } else if let Some(children) = node.children_mut() {
        for child in children {
            let child_found = set_heading_level_from(child, depth, found_any_heading);
            found_any_heading = found_any_heading || child_found;
        }
    }

    found_any_heading
}

fn flatten_headings(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::Heading(heading) = node
        && heading.depth > 1
    {
        let children = node.children().cloned().unwrap_or_default();

        let strong_node = markdown::mdast::Node::Strong(markdown::mdast::Strong {
            children,
            position: None,
        });

        *node = markdown::mdast::Node::Paragraph(markdown::mdast::Paragraph {
            children: vec![strong_node],
            position: None,
        });
    }

    if let Some(children) = node.children_mut() {
        for child in children {
            flatten_headings(child);
        }
    }
}

fn remove_thematic_break(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::ThematicBreak(_) = node {
        *node = markdown::mdast::Node::Paragraph(markdown::mdast::Paragraph {
            children: vec![],
            position: None,
        });
    }

    if let Some(children) = node.children_mut() {
        for child in children {
            remove_thematic_break(child);
        }
    }
}

fn remove_empty_headings(node: &mut markdown::mdast::Node) {
    if let Some(children) = node.children_mut() {
        let mut i = 0;
        while i < children.len() {
            if let Some(next) = children.get(i + 1)
                && matches!(&children[i], markdown::mdast::Node::Heading(_))
                && matches!(next, markdown::mdast::Node::Heading(_))
            {
                children.remove(i);
                continue;
            }
            i += 1;
        }

        for child in children.iter_mut() {
            remove_empty_headings(child);
        }
    }
}

fn add_paragraphs_before_headings(node: &mut markdown::mdast::Node) {
    if let Some(children) = node.children_mut() {
        let mut heading_positions = Vec::new();
        let mut found_first_heading = false;

        for (i, child) in children.iter().enumerate() {
            if let markdown::mdast::Node::Heading(_) = child {
                if found_first_heading {
                    heading_positions.push(i);
                } else {
                    found_first_heading = true;
                }
            }
        }

        for pos in heading_positions.iter().rev() {
            let text_node = markdown::mdast::Node::Text(markdown::mdast::Text {
                value: "\u{00A0}".to_string(),
                position: None,
            });

            let para = markdown::mdast::Node::Paragraph(markdown::mdast::Paragraph {
                children: vec![text_node],
                position: None,
            });

            children.insert(*pos, para);
        }

        for child in children.iter_mut() {
            add_paragraphs_before_headings(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_to_md_1() {
        let input = r#"
# Hello

## World

1. Hi
2. Bye!
"#;

        insta::assert_snapshot!(md_to_md(input).unwrap().to_string(), @"
        # World

        - Hi
        - Bye!
        ");
    }

    #[test]
    fn test_md_to_md_2() {
        let input = r#"
## Hello

### World

1. Hi
2. Bye!
"#;
        insta::assert_snapshot!(md_to_md(input).unwrap().to_string(), @"
        # World

        - Hi
        - Bye!
        ");
    }

    #[test]
    fn test_md_to_md_3() {
        let input = r#"
# Enhanced Meeting Notes
## What Hyprnote Does
- A smart notepad for people with back-to-back meetings.
- Listens to the meeting so you don't have to write everything down.
- Merges your notes and the transcript into a clean, context-aware summary.
- Note-taking is optional but helps highlight what's important to you.

## Privacy and Performance
- Built local-first: works offline and stores data on your device.
- Prioritizes user privacy and seamless experience.

## Flexible and Extendable
- Not limited to specific use cases like sales.
- Simple for anyone to use out of the box.
- Offers powerful extensions—like real-time transcripts and CRM uploads (e.g. Twenty).

## Stay Connected
- Follow updates on [X](https://hyprnote.com/x).
- Join the community and chat on [Discord](https://hyprnote.com/discord). 

# Participants:

* [John Jeong](mailto:john@hyprnote.com)
* [Yujong Lee](mailto:yujonglee@hyprnote.com)

# Meeting Transcript
(No raw excerpt provided, utilized to generate the enhanced note)
"#;

        insta::assert_snapshot!(md_to_md(input).unwrap().to_string(), @"
        # What Hyprnote Does

        - A smart notepad for people with back-to-back meetings.
        - Listens to the meeting so you don't have to write everything down.
        - Merges your notes and the transcript into a clean, context-aware summary.
        - Note-taking is optional but helps highlight what's important to you.

         

        # Privacy and Performance

        - Built local-first: works offline and stores data on your device.
        - Prioritizes user privacy and seamless experience.

         

        # Flexible and Extendable

        - Not limited to specific use cases like sales.
        - Simple for anyone to use out of the box.
        - Offers powerful extensions—like real-time transcripts and CRM uploads (e.g. Twenty).

         

        # Stay Connected

        - Follow updates on [X](https://hyprnote.com/x).
        - Join the community and chat on [Discord](https://hyprnote.com/discord).

         

        # Participants:

        - [John Jeong](mailto:john@hyprnote.com)
        - [Yujong Lee](mailto:yujonglee@hyprnote.com)

         

        # Meeting Transcript

        (No raw excerpt provided, utilized to generate the enhanced note)
        ");
    }

    // TODO: not ideal
    #[test]
    fn test_md_to_md_4() {
        let input = r#"
# Hyprnote: Enhanced Meeting Notes

# Objective: Introduce Hyprnote as a smart notepad for enhanced meeting productivity.
# Privacy & Performance: Built locally, prioritizing user data security and seamless experience.
# Flexible & Extendable: Supports various use cases beyond sales, offering a simple and powerful solution.
# Stay Connected: Promote Hyprnote through X and Discord.

# Key Features:
# - Offline transcription and note-taking.
# - Real-time transcript integration for context.
# - Customizable notes and summaries.
# - Optional extensions for CRM integration (e.g., Twenty).

# Benefits: Streamlines meetings, improves productivity, and enhances data capture.

# Further Information: Follow updates on [X](https://hyprnote.com/x) and [Discord](https://hyprnote.com/discord).
        "#;

        insta::assert_snapshot!(md_to_md(input).unwrap().to_string(), @"# Further Information: Follow updates on [X](https://hyprnote.com/x) and [Discord](https://hyprnote.com/discord).");
    }

    #[test]
    fn test_opinionated_md_to_html() {
        let input = r#"
# Enhanced Meeting Notes
## What Hyprnote Does
- A smart notepad for people with back-to-back meetings.
- Listens to the meeting so you don't have to write everything down.
- Merges your notes and the transcript into a clean, context-aware summary.
- Note-taking is optional but helps highlight what's important to you.

## Privacy and Performance
- Built local-first: works offline and stores data on your device.
- Prioritizes user privacy and seamless experience.

## Flexible and Extendable
- Not limited to specific use cases like sales.
- Simple for anyone to use out of the box.
- Offers powerful extensions—like real-time transcripts and CRM uploads (e.g. Twenty).

## Stay Connected
- Follow updates on [X](https://hyprnote.com/x).
- Join the community and chat on [Discord](https://hyprnote.com/discord).
"#;

        insta::assert_snapshot!(opinionated_md_to_html(input).unwrap().to_string(), @r#"
        <h1>What Hyprnote Does</h1>
        <ul>
        <li>A smart notepad for people with back-to-back meetings.</li>
        <li>Listens to the meeting so you don't have to write everything down.</li>
        <li>Merges your notes and the transcript into a clean, context-aware summary.</li>
        <li>Note-taking is optional but helps highlight what's important to you.</li>
        </ul>
        <p> </p>
        <h1>Privacy and Performance</h1>
        <ul>
        <li>Built local-first: works offline and stores data on your device.</li>
        <li>Prioritizes user privacy and seamless experience.</li>
        </ul>
        <p> </p>
        <h1>Flexible and Extendable</h1>
        <ul>
        <li>Not limited to specific use cases like sales.</li>
        <li>Simple for anyone to use out of the box.</li>
        <li>Offers powerful extensions—like real-time transcripts and CRM uploads (e.g. Twenty).</li>
        </ul>
        <p> </p>
        <h1>Stay Connected</h1>
        <ul>
        <li>Follow updates on <a href="https://hyprnote.com/x">X</a>.</li>
        <li>Join the community and chat on <a href="https://hyprnote.com/discord">Discord</a>.</li>
        </ul>
        "#);
    }

    #[test]
    fn test_mention_transformation() {
        let input =
            r#"Hello @[John Doe](user:john-doe) and @[Jane Smith](workspace:jane-workspace)!"#;

        let html = opinionated_md_to_html(input).unwrap();
        println!("HTML output: {}", html);

        assert!(html.contains(r#"data-mention="true""#));
        assert!(html.contains(r#"data-id="john-doe""#));
        assert!(html.contains(r#"data-type="user""#));
        assert!(html.contains(r#"data-label="John Doe""#));
        assert!(html.contains(r#"@John Doe"#));
        assert!(html.contains(r#"data-id="jane-workspace""#));
        assert!(html.contains(r#"data-type="workspace""#));
        assert!(html.contains(r#"@Jane Smith"#));
    }
}
