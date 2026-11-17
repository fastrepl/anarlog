//! What shipping's open-note dialog lists (`apps/desktop/src/shared/open-note-dialog.tsx`):
//! "Go to" pages, recently opened notes and all notes, filtered by title, with the same group
//! order, separators and spacing, and cmdk's keyboard movement between the items.

use std::sync::Arc;

use desktop_runtime::SessionId;

use super::navigation::Route;
use crate::product::navigation::{PAGES, Page, opens_workspace};

/// Recently opened notes kept (and persisted), newest first; the dialog shows the first five.
pub const MAX_RECENT: usize = 10;
const MAX_RECENT_DISPLAY: usize = 5;

/// Shipping appends this entry to the settings navigation in its page results.
const SETTINGS: Page = Page {
    id: "settings",
    group: "Go to",
    label: "Settings",
    icon: "product/Settings02Icon.svg",
    terms: &[],
    requires_pro: false,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// A local session or a shared note.
    pub route: Route,
    pub title: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A group heading, with the separator line above it when another group precedes it.
    Heading {
        label: &'static str,
        separator: bool,
    },
    Page(Page),
    Note {
        note: Note,
        recent: bool,
    },
    /// The `pb-1.5` closing a group that another group follows.
    Gap,
}

impl Row {
    pub fn selectable(&self) -> bool {
        matches!(self, Self::Page(_) | Self::Note { .. })
    }

    /// cmdk's item value, so a selection survives a reload of the same results.
    pub fn key(&self) -> Option<String> {
        Some(match self {
            Self::Page(page) => format!("page-{}", page.id),
            Self::Note { note, recent } => match &note.route {
                Route::Session(id) if *recent => format!("recent-{}", id.0),
                Route::Session(id) => format!("session-{}", id.0),
                Route::SharedSession(id) => format!("shared_session-{id}"),
                _ => return None,
            },
            Self::Heading { .. } | Self::Gap => return None,
        })
    }

    /// Where choosing the row goes, and whether it opens in a new tab: pages open a new tab,
    /// notes replace the current one.
    pub fn destination(&self) -> Option<(Route, bool)> {
        match self {
            Self::Page(page) => Some((page_route(*page), true)),
            Self::Note { note, .. } => Some((note.route.clone(), false)),
            Self::Heading { .. } | Self::Gap => None,
        }
    }
}

/// Settings pages carry a "Settings" hint; pages that open a workspace view do not.
pub fn hint(page: Page) -> Option<&'static str> {
    (page.id != SETTINGS.id && !opens_workspace(page)).then_some("Settings")
}

fn page_route(page: Page) -> Route {
    match page.id {
        "settings" => Route::Settings("app".into()),
        "folders" => Route::Folders,
        "calendar" => Route::Calendar,
        "contacts" => Route::Contacts,
        "templates" => Route::Templates,
        "automations" => Route::Automations,
        id => Route::Settings(id.into()),
    }
}

/// The dialog's rows for `query`. `notes` are newest first; `recent` is newest first.
pub fn rows(notes: &[Note], recent: &[SessionId], query: &str) -> Vec<Row> {
    let trimmed = query.trim().to_lowercase();
    let empty = trimmed.is_empty();
    let pages = PAGES
        .iter()
        .copied()
        .chain([SETTINGS])
        .filter(|page| {
            if empty {
                page.id == SETTINGS.id || opens_workspace(*page)
            } else {
                [Some(page.label), Some(page.group), hint(*page)]
                    .into_iter()
                    .flatten()
                    .any(|text| text.to_lowercase().contains(&trimmed))
            }
        })
        .collect::<Vec<_>>();
    // Shipping matches note titles against the untrimmed query.
    let lower = query.to_lowercase();
    let matches = |note: &&Note| empty || note.title.to_lowercase().contains(&lower);
    let recent = recent
        .iter()
        .take(MAX_RECENT_DISPLAY)
        .filter_map(|id| {
            let route = Route::Session(id.clone());
            notes.iter().find(|note| note.route == route)
        })
        .collect::<Vec<_>>();
    let others = notes
        .iter()
        .filter(|note| !recent.iter().any(|recent| recent.route == note.route))
        .filter(matches)
        .collect::<Vec<_>>();
    let recent = recent.into_iter().filter(matches).collect::<Vec<_>>();

    let mut rows = Vec::new();
    let go_to = |rows: &mut Vec<Row>, separator: bool, gap: bool| {
        if pages.is_empty() {
            return;
        }
        rows.push(Row::Heading {
            label: "Go to",
            separator,
        });
        rows.extend(pages.iter().copied().map(Row::Page));
        if gap {
            rows.push(Row::Gap);
        }
    };
    if !empty {
        go_to(&mut rows, false, !recent.is_empty() || !others.is_empty());
    }
    if !recent.is_empty() {
        rows.push(Row::Heading {
            label: "Recent",
            separator: !empty && !pages.is_empty(),
        });
        rows.extend(recent.iter().map(|note| Row::Note {
            note: (*note).clone(),
            recent: true,
        }));
        if !others.is_empty() || (empty && !pages.is_empty()) {
            rows.push(Row::Gap);
        }
    }
    if empty {
        go_to(&mut rows, !recent.is_empty(), !others.is_empty());
    }
    if !others.is_empty() {
        rows.push(Row::Heading {
            label: "All Notes",
            separator: !pages.is_empty() || !recent.is_empty(),
        });
        rows.extend(others.into_iter().map(|note| Row::Note {
            note: note.clone(),
            recent: false,
        }));
    }
    rows
}

#[derive(Clone, Copy, Debug)]
pub enum Move {
    Next,
    Previous,
    First,
    Last,
    /// Alt+arrow: the first item of the next or previous group.
    NextGroup,
    PreviousGroup,
}

/// cmdk's selection movement: no wrapping, and group jumps fall back to a single step.
pub fn step(rows: &[Row], selected: Option<usize>, movement: Move) -> Option<usize> {
    let items = (0..rows.len())
        .filter(|&index| rows[index].selectable())
        .collect::<Vec<_>>();
    let position = selected.and_then(|selected| items.iter().position(|&index| index == selected));
    let by = |offset: isize| {
        let target = position.map_or(offset - 1, |position| position as isize + offset);
        usize::try_from(target)
            .ok()
            .and_then(|target| items.get(target).copied())
            .or(selected)
    };
    let heading = |index: usize| {
        (0..=index)
            .rev()
            .find(|&row| matches!(rows[row], Row::Heading { .. }))
    };
    let first_after = |row: usize| items.iter().copied().find(|&item| item > row);
    match movement {
        Move::Next => by(1),
        Move::Previous => by(-1),
        Move::First => items.first().copied().or(selected),
        Move::Last => items.last().copied().or(selected),
        Move::NextGroup => selected
            .and_then(|selected| {
                (selected..rows.len()).find(|&row| matches!(rows[row], Row::Heading { .. }))
            })
            .and_then(first_after)
            .or_else(|| by(1)),
        Move::PreviousGroup => selected
            .and_then(heading)
            .and_then(|current| current.checked_sub(1))
            .and_then(heading)
            .and_then(first_after)
            .or_else(|| by(-1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str, title: &str) -> Note {
        Note {
            route: Route::Session(SessionId(id.into())),
            title: title.into(),
        }
    }

    fn library() -> Vec<Note> {
        vec![
            note("a", "Weekly product sync"),
            Note {
                route: Route::SharedSession("share".into()),
                title: "Shared roadmap".into(),
            },
            note("b", "Customer call"),
            note("c", "Hiring debrief"),
        ]
    }

    fn outline(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Heading { label, separator } => {
                    format!("{}{label}", if *separator { "--" } else { "" })
                }
                Row::Page(page) => format!("page:{}", page.id),
                Row::Note { note, .. } => format!("{}", note.title),
                Row::Gap => "gap".into(),
            })
            .collect()
    }

    #[test]
    fn empty_query_lists_recent_then_top_level_pages_then_the_remaining_notes() {
        let rows = rows(
            &library(),
            &[SessionId("c".into()), SessionId("gone".into())],
            "",
        );
        assert_eq!(
            outline(&rows),
            [
                "Recent",
                "Hiring debrief",
                "gap",
                "--Go to",
                "page:folders",
                "page:calendar",
                "page:contacts",
                "page:templates",
                "page:automations",
                "page:settings",
                "gap",
                "--All Notes",
                "Weekly product sync",
                "Shared roadmap",
                "Customer call",
            ]
        );
        assert_eq!(rows[1].key().unwrap(), "recent-c");
        assert_eq!(
            rows[4].destination(),
            Some((Route::Folders, true)),
            "pages open a new tab"
        );
        assert_eq!(
            rows[12].destination(),
            Some((Route::Session(SessionId("a".into())), false))
        );
    }

    #[test]
    fn query_matches_page_labels_groups_and_hints_before_notes_by_title() {
        let recent = [SessionId("a".into())];
        assert_eq!(
            outline(&rows(&library(), &recent, "SYNC")),
            [
                "Go to",
                "page:sync",
                "gap",
                "--Recent",
                "Weekly product sync"
            ]
        );
        // Pages match the trimmed query; notes match it untrimmed, like shipping.
        assert_eq!(
            outline(&rows(&library(), &recent, " sync ")),
            ["Go to", "page:sync"]
        );
        assert_eq!(
            outline(&rows(&library(), &recent, "roadmap")),
            ["All Notes", "Shared roadmap"]
        );
        let settings = rows(&library(), &[], "settings");
        assert_eq!(settings.iter().filter(|row| row.selectable()).count(), 20);
        assert_eq!(
            settings[1].destination(),
            Some((Route::Settings("app".into()), true))
        );
        assert!(rows(&library(), &recent, "nothing like this").is_empty());
    }

    #[test]
    fn keyboard_moves_like_cmdk_without_wrapping() {
        let rows = rows(&library(), &[SessionId("c".into())], "");
        let first = step(&rows, None, Move::Next);
        assert_eq!(first, Some(1));
        assert_eq!(step(&rows, first, Move::Previous), first);
        assert_eq!(step(&rows, first, Move::Next), Some(4));
        assert_eq!(step(&rows, Some(4), Move::NextGroup), Some(12));
        assert_eq!(step(&rows, Some(12), Move::PreviousGroup), Some(4));
        assert_eq!(step(&rows, Some(4), Move::PreviousGroup), Some(1));
        assert_eq!(step(&rows, Some(14), Move::NextGroup), Some(14));
        assert_eq!(step(&rows, Some(4), Move::Last), Some(14));
        assert_eq!(step(&rows, Some(14), Move::Next), Some(14));
        assert_eq!(step(&rows, Some(14), Move::First), Some(1));
    }
}
