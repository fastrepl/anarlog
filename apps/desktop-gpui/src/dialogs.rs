//! Native file and folder choosers. The Tauri app's dialogs are GTK's — the
//! dialog plugin (rfd) and WebKitGTK's `<input type="file">` both open a
//! `GtkFileChooserDialog` with Cancel / Open — so the shell opens the same
//! dialog on its GTK thread. gpui's own prompt goes through the XDG portal
//! over zbus, which needs a portal and a tokio context this process's UI
//! thread has neither of.

use std::path::PathBuf;

/// `filters: [{ name, extensions }]` of the dialog plugin, or an `accept`
/// list of MIME patterns for a file input (`image/*`).
#[derive(Debug, Clone)]
pub enum Filter {
    Extensions {
        name: &'static str,
        extensions: &'static [&'static str],
    },
    Mime(&'static str),
}

/// What the dialog picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    File,
    Files,
    Folder,
}

pub struct Options {
    pub title: String,
    pub pick: Pick,
    /// `defaultPath`: the folder the dialog opens in.
    pub start_dir: Option<PathBuf>,
    pub filters: Vec<Filter>,
}

/// tao's `set_theme`: `gtk-application-prefer-dark-theme` follows the app's
/// resolved theme, so the GTK dialogs opened from here match it. Only GTK
/// has the setting.
pub fn set_prefer_dark(dark: bool) {
    #[cfg(target_os = "linux")]
    if crate::gtk_loop::ensure_running() {
        crate::gtk_loop::invoke(move || {
            use gtk::prelude::*;
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_application_prefer_dark_theme(dark);
            }
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = dark;
}

/// `MessageDialogButtons::OkCancelCustom(ok, cancel)` with the builder's
/// default kind, `Info`.
pub struct MessageOptions {
    pub title: String,
    pub description: String,
    pub ok: String,
    pub cancel: String,
}

/// `app.dialog().message(description).title(title).buttons(OkCancelCustom).show`:
/// resolves `true` when the ok button was chosen.
pub fn message(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    options: MessageOptions,
) -> impl std::future::Future<Output = bool> + use<> {
    let (sender, receiver) = tokio::sync::oneshot::channel::<bool>();
    platform::open_message(window, cx, options, sender);
    async move { receiver.await.unwrap_or(false) }
}

/// Opens the chooser and resolves with the chosen paths, `None` when the
/// dialog is cancelled or cannot be shown.
pub fn pick(
    cx: &mut gpui::App,
    options: Options,
) -> impl std::future::Future<Output = Option<Vec<PathBuf>>> + use<> {
    let (sender, receiver) = tokio::sync::oneshot::channel::<Option<Vec<PathBuf>>>();
    platform::open(cx, options, sender);
    async move { receiver.await.ok().flatten() }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Filter, MessageOptions, Options, Pick};
    use gtk::prelude::*;
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use tokio::sync::oneshot::Sender;

    pub fn open(_cx: &mut gpui::App, options: Options, sender: Sender<Option<Vec<PathBuf>>>) {
        if !crate::gtk_loop::ensure_running() {
            let _ = sender.send(None);
            return;
        }
        crate::gtk_loop::invoke(move || {
            let action = match options.pick {
                Pick::Folder => gtk::FileChooserAction::SelectFolder,
                Pick::File | Pick::Files => gtk::FileChooserAction::Open,
            };
            // rfd's GTK dialog: Cancel and Open, no parent window.
            let dialog = gtk::FileChooserDialog::with_buttons::<gtk::Window>(
                Some(options.title.as_str()),
                None,
                action,
                &[
                    ("Cancel", gtk::ResponseType::Cancel),
                    ("Open", gtk::ResponseType::Accept),
                ],
            );
            dialog.set_select_multiple(options.pick == Pick::Files);
            if let Some(start) = options.start_dir.as_deref() {
                dialog.set_current_folder(start);
            }
            for filter in &options.filters {
                let gtk_filter = gtk::FileFilter::new();
                match filter {
                    Filter::Extensions { name, extensions } => {
                        gtk_filter.set_name(Some(name));
                        for extension in extensions.iter() {
                            gtk_filter.add_pattern(&format!("*.{extension}"));
                        }
                    }
                    Filter::Mime(mime) => {
                        gtk_filter.add_mime_type(mime);
                    }
                }
                dialog.add_filter(gtk_filter);
            }
            let sender = Rc::new(Cell::new(Some(sender)));
            dialog.connect_response(move |dialog, response| {
                let paths = (response == gtk::ResponseType::Accept).then(|| dialog.filenames());
                if let Some(sender) = sender.take() {
                    let _ = sender.send(paths.filter(|paths| !paths.is_empty()));
                }
                dialog.close();
            });
            dialog.show();
        });
    }

    /// rfd's GTK message dialog: `gtk_message_dialog_new(NULL, MODAL, INFO,
    /// BUTTONS_NONE, "%s", title)` with the description as the secondary
    /// text, the title on the window too, selectable labels and the custom
    /// buttons in order.
    pub fn open_message(
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
        options: MessageOptions,
        sender: Sender<bool>,
    ) {
        if !crate::gtk_loop::ensure_running() {
            let _ = sender.send(false);
            return;
        }
        crate::gtk_loop::invoke(move || {
            let dialog = gtk::MessageDialog::new(
                None::<&gtk::Window>,
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Info,
                gtk::ButtonsType::None,
                &options.title,
            );
            dialog.set_secondary_text(Some(&options.description));
            dialog.set_title(&options.title);
            if let Ok(area) = dialog.message_area().downcast::<gtk::Container>() {
                for child in area.children() {
                    if let Ok(label) = child.downcast::<gtk::Label>() {
                        label.set_selectable(true);
                    }
                }
            }
            dialog.add_button(&options.ok, gtk::ResponseType::Ok);
            dialog.add_button(&options.cancel, gtk::ResponseType::Cancel);
            let sender = Rc::new(Cell::new(Some(sender)));
            dialog.connect_response(move |dialog, response| {
                if let Some(sender) = sender.take() {
                    let _ = sender.send(response == gtk::ResponseType::Ok);
                }
                dialog.close();
            });
            dialog.show();
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{MessageOptions, Options, Pick};
    use std::path::PathBuf;
    use tokio::sync::oneshot::Sender;

    /// gpui's prompt is the platform's own alert on macOS and Windows.
    pub fn open_message(
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        options: MessageOptions,
        sender: Sender<bool>,
    ) {
        let answer = window.prompt(
            gpui::PromptLevel::Info,
            &options.title,
            Some(&options.description),
            &[options.ok.as_str(), options.cancel.as_str()],
            cx,
        );
        cx.spawn(async move |_| {
            let _ = sender.send(answer.await == Ok(0));
        })
        .detach();
    }

    /// gpui's prompt is the platform's own dialog on macOS and Windows.
    pub fn open(cx: &mut gpui::App, options: Options, sender: Sender<Option<Vec<PathBuf>>>) {
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: options.pick != Pick::Folder,
            directories: options.pick == Pick::Folder,
            multiple: options.pick == Pick::Files,
            prompt: Some(options.title.into()),
        });
        cx.spawn(async move |_| {
            let paths = match picker.await {
                Ok(Ok(paths)) => paths,
                _ => None,
            };
            let _ = sender.send(paths);
        })
        .detach();
    }
}
