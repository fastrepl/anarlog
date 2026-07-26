use markdown::mdast;

pub fn mdast_to_markdown(node: &mdast::Node) -> Result<String, String> {
    let task_items = collect_task_items(node);

    let md = mdast_util_to_markdown::to_markdown_with_options(
        node,
        &mdast_util_to_markdown::Options {
            bullet: '-',
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;

    let md = inject_task_checkboxes(&md, &task_items);
    Ok(unescape_markdown(&md))
}

fn unescape_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
            && is_markdown_escapable(next)
        {
            result.push(next);
            chars.next();
            continue;
        }
        result.push(c);
    }

    result
}

fn is_markdown_escapable(c: char) -> bool {
    matches!(
        c,
        '\\' | '`'
            | '*'
            | '_'
            | '{'
            | '}'
            | '['
            | ']'
            | '('
            | ')'
            | '#'
            | '+'
            | '-'
            | '.'
            | '!'
            | '|'
            | '<'
            | '>'
    )
}

fn collect_task_items(node: &mdast::Node) -> Vec<Option<bool>> {
    let mut items = Vec::new();
    collect_task_items_recursive(node, &mut items);
    items
}

fn collect_task_items_recursive(node: &mdast::Node, items: &mut Vec<Option<bool>>) {
    match node {
        mdast::Node::Root(root) => {
            for child in &root.children {
                collect_task_items_recursive(child, items);
            }
        }
        mdast::Node::List(list) => {
            for child in &list.children {
                if let mdast::Node::ListItem(item) = child {
                    items.push(item.checked);
                    collect_task_items_recursive(child, items);
                }
            }
        }
        mdast::Node::ListItem(item) => {
            for child in &item.children {
                collect_task_items_recursive(child, items);
            }
        }
        mdast::Node::Blockquote(bq) => {
            for child in &bq.children {
                collect_task_items_recursive(child, items);
            }
        }
        _ => {}
    }
}

fn inject_task_checkboxes(md: &str, task_items: &[Option<bool>]) -> String {
    let mut result = String::with_capacity(md.len() + task_items.len() * 6);
    let mut task_index = 0;

    for line in md.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if task_index < task_items.len() {
                if let Some(checked) = task_items[task_index] {
                    let prefix_len = line.len() - trimmed.len();
                    let bullet = &trimmed[..2];
                    let rest = &trimmed[2..];
                    let checkbox = if checked { "[x] " } else { "[ ] " };
                    result.push_str(&line[..prefix_len]);
                    result.push_str(bullet);
                    result.push_str(checkbox);
                    result.push_str(rest);
                } else {
                    result.push_str(line);
                }
                task_index += 1;
            } else {
                result.push_str(line);
            }
        } else if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            let mut chars = trimmed.chars().peekable();
            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                chars.next();
            }
            if chars.next() == Some('.')
                && chars.next() == Some(' ')
                && task_index < task_items.len()
            {
                task_index += 1;
            }
            result.push_str(line);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    if result.ends_with('\n') && !md.ends_with('\n') {
        result.pop();
    }

    result
}
