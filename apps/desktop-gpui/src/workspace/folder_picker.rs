//! Shipping's folder picker (`session/components/folder-picker.tsx`): search, the folders with
//! the current one checked, `Create "…"` and "See all folders". The note header's folder chip
//! and the calendar event popover both show it.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle, SessionId};
use gpui::{
    Context, Entity, EventEmitter, KeyDownEvent, Render, SharedString, Subscription, Window, div,
    prelude::*, px,
};

use super::{mutations::failure, resource_list::Glyph};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::{Theme, theme},
};

/// Folder paths the shipping picker offers: explicit folders plus folders that only exist on
/// notes or attachments. `known` marks rows in `folders`.
pub const FOLDER_QUERY: &str = "SELECT path, 1 AS known, icon_json FROM folders WHERE deleted_at IS NULL AND path <> ''
    UNION SELECT folder_path, 0, NULL FROM sessions WHERE deleted_at IS NULL AND folder_path <> ''
    UNION SELECT folder_path, 0, NULL FROM folder_attachments WHERE deleted_at IS NULL AND folder_path <> ''
    LIMIT 2000";

#[derive(Debug, Default, PartialEq)]
pub struct FolderList {
    pub paths: Vec<Arc<str>>,
    known: HashSet<Arc<str>>,
    icons: HashMap<Arc<str>, Glyph>,
}

impl FolderList {
    /// Normalizes paths, adds their ancestors and sorts them like shipping's
    /// `collectFolderPaths`. Rows are `(path, known, icon_json)`.
    pub fn new(rows: impl IntoIterator<Item = (String, bool, Option<String>)>) -> Self {
        let mut paths = HashSet::<Arc<str>>::new();
        let mut known = HashSet::new();
        let mut icons = HashMap::new();
        for (path, explicit, icon) in rows {
            let Ok(path) = anlg_fs_sync_core::normalize_folder_path(&path) else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            let path: Arc<str> = path.as_str().into();
            if explicit {
                known.insert(path.clone());
            }
            if let Some(icon) = icon {
                icons.insert(path.clone(), Glyph::folder(&icon));
            }
            let mut prefix = String::new();
            for segment in path.split('/') {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(segment);
                paths.insert(prefix.as_str().into());
            }
        }
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        paths.sort_by(|a, b| {
            a.to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| b.cmp(a))
        });
        Self {
            paths,
            known,
            icons,
        }
    }

    pub fn known(&self, path: &str) -> bool {
        self.known.contains(path)
    }

    fn icon(&self, path: &str) -> Glyph {
        self.icons
            .get(path)
            .cloned()
            .unwrap_or_else(Glyph::folder_default)
    }
}

pub fn load(runtime: &RuntimeHandle) -> Result<desktop_runtime::Reply<Arc<FolderList>>> {
    runtime.read(CancellationToken::new(), |services| async move {
        let rows = services
            .executor
            .execute(FOLDER_QUERY.into(), vec![])
            .await
            .map_err(failure)?;
        Ok(Arc::new(FolderList::new(rows.iter().map(|row| {
            (
                row["path"].as_str().unwrap_or_default().to_owned(),
                row["known"] == 1,
                row["icon_json"].as_str().map(str::to_owned),
            )
        }))))
    })
}

/// Files notes into `folder` ("" takes them out of their folder), creating its catalog row
/// first when the picker offered it as new.
pub async fn move_notes(
    runtime: &RuntimeHandle,
    ids: Arc<[SessionId]>,
    folder: Arc<str>,
    create: bool,
) -> Result<()> {
    if create && !folder.is_empty() {
        super::folders::create(runtime, folder.clone())?
            .receive()
            .await?;
    }
    super::notes::dispatch(runtime, super::notes::NoteCommand::Move { ids, folder })?
        .receive()
        .await
}

pub enum FolderPickerEvent {
    /// `path` is "" when the current folder was chosen again, which takes the note out of it.
    Choose {
        path: Arc<str>,
        create: bool,
    },
    SeeAll,
    Dismiss,
}

#[derive(Clone, Debug, PartialEq)]
enum Row {
    Folder(Arc<str>),
    Create(Arc<str>),
}

pub struct FolderPicker {
    runtime: RuntimeHandle,
    search: Entity<TextInput>,
    folders: Option<Arc<FolderList>>,
    current: Arc<str>,
    highlighted: Option<Row>,
    error: Option<String>,
    _search: Subscription,
}

impl EventEmitter<FolderPickerEvent> for FolderPicker {}

impl FolderPicker {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search or create folder", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, event, cx| {
            if let InputEvent::Submitted = event {
                if let Some(row) = this.selected(cx) {
                    this.choose(row, cx);
                }
            } else {
                // cmdk keeps the highlighted value while it stays visible, else the first.
                cx.notify();
            }
        });
        Self {
            runtime,
            search,
            folders: None,
            current: "".into(),
            highlighted: None,
            error: None,
            _search: subscription,
        }
    }

    /// Resets the search for a note (or rule) whose folder is `current` and reloads folders.
    pub fn open(&mut self, current: Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        // Shipping's list starts on its first row.
        self.highlighted = None;
        self.current = current;
        self.error = None;
        self.search
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.focus_view(&self.search, window);
        let reply = load(&self.runtime);
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(folders) => this.folders = Some(folders),
                    Err(error) => this.error = Some(format!("Could not load folders: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn rows(&self, cx: &gpui::App) -> (Vec<Row>, &'static str) {
        let typed = self.search.read(cx).buffer.text.trim().to_owned();
        let query = typed.to_lowercase();
        let name = anlg_fs_sync_core::normalize_folder_path(&typed).ok();
        let list = self.folders.clone().unwrap_or_default();
        let mut paths = list.paths.clone();
        // Shipping lists a current folder that no longer exists anywhere else, too.
        if !self.current.is_empty() && !paths.contains(&self.current) {
            paths.push(self.current.clone());
            paths.sort_by_key(|path| path.to_lowercase());
        }
        let mut rows: Vec<Row> = paths
            .iter()
            .filter(|path| path.to_lowercase().contains(&query))
            .cloned()
            .map(Row::Folder)
            .collect();
        if let Some(name) = name
            .as_deref()
            .filter(|name| !name.is_empty() && !paths.iter().any(|path| &**path == *name))
        {
            rows.push(Row::Create(name.into()));
        }
        let empty = if query.is_empty() {
            "No folders yet."
        } else if name.is_none() {
            "Enter a valid folder name."
        } else {
            "No folders found."
        };
        (rows, empty)
    }

    fn selected(&self, cx: &gpui::App) -> Option<Row> {
        let (rows, _) = self.rows(cx);
        self.highlighted
            .clone()
            .filter(|row| rows.contains(row))
            .or_else(|| rows.first().cloned())
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let (rows, _) = self.rows(cx);
        let Some(selected) = self.selected(cx) else {
            return;
        };
        let index = rows.iter().position(|row| *row == selected).unwrap_or(0);
        let next = if forward {
            (index + 1).min(rows.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        self.highlighted = rows.get(next).cloned();
        cx.notify();
    }

    fn choose(&mut self, row: Row, cx: &mut Context<Self>) {
        let (path, typed_new) = match row {
            Row::Folder(path) if path == self.current => ("".into(), false),
            Row::Folder(path) => (path, false),
            Row::Create(path) => (path, true),
        };
        if path == self.current {
            cx.emit(FolderPickerEvent::Dismiss);
            return;
        }
        let create = typed_new
            || (!path.is_empty() && !self.folders.as_ref().is_some_and(|list| list.known(&path)));
        cx.emit(FolderPickerEvent::Choose { path, create });
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "up" => self.step(false, cx),
            "down" => self.step(true, cx),
            // Escape also reaches the owner, which may close more than the picker.
            "escape" => return cx.emit(FolderPickerEvent::Dismiss),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn row(
        &self,
        row: &Row,
        selected: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (id, glyph, label, checked) = match row {
            Row::Folder(path) => (
                SharedString::from(format!("folder-picker:{path}")),
                div()
                    .opacity(0.7)
                    .child(
                        self.folders
                            .as_ref()
                            .map_or_else(Glyph::folder_default, |list| list.icon(path))
                            .render(16.),
                    )
                    .into_any_element(),
                SharedString::from(path.clone()),
                *path == self.current,
            ),
            Row::Create(name) => (
                SharedString::from("folder-picker-create"),
                icon("workspace/PlusIcon.svg", 16., colors.foreground).into_any_element(),
                SharedString::from(format!("Create \"{name}\"")),
                false,
            ),
        };
        let hovered = row.clone();
        let chosen = row.clone();
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py(px(6.))
            .text_sm()
            .line_height(px(20.))
            .text_color(colors.foreground)
            .cursor_pointer()
            .when(selected, |row| row.bg(colors.accent))
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                if *hovering {
                    this.highlighted = Some(hovered.clone());
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.choose(chosen.clone(), cx)))
            .child(glyph)
            .child(div().flex_1().min_w_0().truncate().child(label))
            .when(checked, |row| {
                row.child(icon("workspace/CheckIcon.svg", 16., colors.foreground))
            })
            .into_any_element()
    }
}

impl Render for FolderPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let (rows, empty) = self.rows(cx);
        let selected = self.selected(cx);
        let loaded = self.folders.is_some();
        div()
            .id("folder-picker")
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(Self::key))
            .child(
                div()
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(32.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(
                                icon("Search01Icon.svg", 16., colors.foreground)
                                    .mr_2()
                                    .opacity(0.5),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .child(self.search.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id("folder-picker-list")
                            .max_h(px(300.))
                            .overflow_y_scroll()
                            // `CommandList` and `CommandGroup` each pad by 4px.
                            .p_2()
                            .when_some(self.error.clone(), |list, error| {
                                list.child(
                                    div()
                                        .px(px(10.))
                                        .py_2()
                                        .text_sm()
                                        .text_color(colors.muted_foreground)
                                        .child(error),
                                )
                            })
                            .when(loaded && rows.is_empty(), |list| {
                                list.child(
                                    div()
                                        .px(px(10.))
                                        .py_2()
                                        .text_sm()
                                        .text_color(colors.muted_foreground)
                                        .child(empty),
                                )
                            })
                            .children(
                                rows.iter()
                                    .map(|row| {
                                        self.row(row, selected.as_ref() == Some(row), colors, cx)
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                    ),
            )
            .child(
                div()
                    .id("folder-picker-all")
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .px_3()
                    .py(px(6.))
                    .text_xs()
                    .line_height(px(16.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.accent).text_color(colors.foreground))
                    .child("See all folders")
                    .child(icon(
                        "workspace/ArrowRight01Icon.svg",
                        14.,
                        colors.muted_foreground,
                    ))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(FolderPickerEvent::SeeAll))),
            )
    }
}

fn icon(path: &'static str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_picker_offers_implicit_folders_and_their_parents() {
        let list = FolderList::new([
            ("Clients/Acme".to_owned(), false, None),
            ("research".to_owned(), true, None),
            ("/research/".to_owned(), false, None),
            ("Zeta".to_owned(), true, None),
        ]);
        assert_eq!(
            list.paths.iter().map(|path| &**path).collect::<Vec<_>>(),
            ["Clients", "Clients/Acme", "research", "Zeta"]
        );
        assert!(list.known("research"));
        assert!(!list.known("Clients/Acme"));
    }

    #[test]
    fn choosing_a_new_folder_creates_it_and_the_empty_choice_unfiles_the_note() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
                database: directory.path().join("library.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let sql = |sql: &'static str| {
                let runtime = runtime.clone();
                async move {
                    runtime
                        .submit(move |services| async move {
                            services
                                .executor
                                .execute(sql.into(), vec![])
                                .await
                                .map_err(failure)
                        })
                        .unwrap()
                        .receive()
                        .await
                        .unwrap()
                }
            };
            sql("INSERT INTO sessions(id,title) VALUES ('s','Sync')").await;
            let id: Arc<[SessionId]> = vec![SessionId("s".into())].into();
            move_notes(&runtime, id.clone(), "Clients/Acme".into(), true)
                .await
                .unwrap();
            let folders = load(&runtime).unwrap().receive().await.unwrap();
            assert!(folders.known("Clients/Acme"));
            assert_eq!(
                sql("SELECT folder_path FROM sessions WHERE id='s'").await[0]["folder_path"],
                "Clients/Acme"
            );
            move_notes(&runtime, id, "".into(), false).await.unwrap();
            assert_eq!(
                sql("SELECT folder_path FROM sessions WHERE id='s'").await[0]["folder_path"],
                ""
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
