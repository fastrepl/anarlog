use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::error::{CliError, CliResult};
use crate::tui::InlineViewport;
use crate::tui::InputAction;

const SKILL_CONTENT: &str = include_str!("../../../../../skills/cli/SKILL.md");

#[derive(Clone, Copy)]
enum SkillFormat {
    ClaudeCode,
    Codex,
    GitHubCopilot,
}

impl SkillFormat {
    const ALL: [SkillFormat; 3] = [
        SkillFormat::ClaudeCode,
        SkillFormat::Codex,
        SkillFormat::GitHubCopilot,
    ];

    fn label(self) -> &'static str {
        match self {
            SkillFormat::ClaudeCode => "Claude Code",
            SkillFormat::Codex => "Codex",
            SkillFormat::GitHubCopilot => "GitHub Copilot",
        }
    }

    fn dir(self) -> &'static str {
        match self {
            SkillFormat::ClaudeCode => ".claude/skills/char",
            SkillFormat::Codex => ".codex/skills/char",
            SkillFormat::GitHubCopilot => ".github/skills/char",
        }
    }

    fn cli_value(self) -> &'static str {
        match self {
            SkillFormat::ClaudeCode => "claude",
            SkillFormat::Codex => "codex",
            SkillFormat::GitHubCopilot => "github-copilot",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(SkillFormat::ClaudeCode),
            "codex" => Some(SkillFormat::Codex),
            "github-copilot" => Some(SkillFormat::GitHubCopilot),
            _ => None,
        }
    }
}

const HIGHLIGHT: Color = Color::Rgb(0xFD, 0xE6, 0xAE);

fn selector_lines(selected: usize) -> Vec<Line<'static>> {
    let mut lines = vec![];

    for (i, format) in SkillFormat::ALL.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            Style::default().fg(HIGHLIGHT)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format.label(), style),
            Span::raw("  "),
            Span::styled(
                format!("./{}/", format.dir()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines
}

fn write_skill(format: SkillFormat) -> CliResult<PathBuf> {
    let dir = PathBuf::from(format.dir());
    std::fs::create_dir_all(&dir)
        .map_err(|e| CliError::operation_failed("create skill directory", e.to_string()))?;

    let path = dir.join("SKILL.md");
    std::fs::write(&path, SKILL_CONTENT)
        .map_err(|e| CliError::operation_failed("write SKILL.md", e.to_string()))?;

    Ok(path)
}

pub fn run(format_arg: Option<&str>) -> CliResult<()> {
    if let Some(name) = format_arg {
        let format = SkillFormat::from_str(name).ok_or_else(|| {
            CliError::invalid_argument(
                "--format",
                name,
                format!(
                    "expected one of: {}",
                    SkillFormat::ALL
                        .iter()
                        .map(|f| f.cli_value())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })?;
        let path = write_skill(format)?;
        eprintln!("Installed char skill at {}", path.display());
        return Ok(());
    }

    if !std::io::stderr().is_terminal() {
        return Err(CliError::operation_failed(
            "skill install",
            "interactive selection requires a terminal; use --format to specify".to_string(),
        ));
    }

    let mut selected: usize = 0;
    let count = SkillFormat::ALL.len();

    let mut viewport = InlineViewport::stderr_interactive(3, None, true)
        .map_err(|e| CliError::operation_failed("create viewport", e.to_string()))?;

    viewport.draw(&selector_lines(selected));

    loop {
        std::thread::sleep(Duration::from_millis(30));

        for action in viewport.poll_input() {
            match action {
                InputAction::Up => {
                    selected = if selected == 0 {
                        count - 1
                    } else {
                        selected - 1
                    };
                }
                InputAction::Down => {
                    selected = (selected + 1) % count;
                }
                InputAction::Confirm => {
                    viewport
                        .clear()
                        .map_err(|e| CliError::operation_failed("clear viewport", e.to_string()))?;

                    let format = SkillFormat::ALL[selected];
                    let path = write_skill(format)?;
                    eprintln!("Installed char skill at {}", path.display());
                    return Ok(());
                }
                _ => {}
            }
        }

        viewport.draw(&selector_lines(selected));
    }
}
