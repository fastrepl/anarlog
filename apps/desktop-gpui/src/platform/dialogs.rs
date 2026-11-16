use std::path::PathBuf;

use desktop_runtime::{Result, ServiceError};
use futures::future::LocalBoxFuture;
use gpui::{App, PathPromptOptions};

#[derive(Clone, Debug)]
pub struct FileFilter {
    pub label: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Picker {
    pub directory: bool,
    pub initial_directory: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub prompt: String,
}

pub trait DialogAdapter {
    fn pick(
        &self,
        request: Picker,
        cx: &mut App,
    ) -> LocalBoxFuture<'static, Result<Option<PathBuf>>>;
}

pub struct GpuiPicker;

pub struct NativePicker;

impl DialogAdapter for NativePicker {
    fn pick(
        &self,
        request: Picker,
        _: &mut App,
    ) -> LocalBoxFuture<'static, Result<Option<PathBuf>>> {
        let mut dialog = rfd::AsyncFileDialog::new().set_title(request.prompt);
        if let Some(directory) = request.initial_directory {
            dialog = dialog.set_directory(directory);
        }
        for filter in request.filters {
            dialog = dialog.add_filter(filter.label, &filter.extensions);
        }
        Box::pin(async move {
            let selected = if request.directory {
                dialog.pick_folder().await
            } else {
                dialog.pick_file().await
            };
            Ok(selected.map(|file| file.path().to_path_buf()))
        })
    }
}

impl DialogAdapter for GpuiPicker {
    fn pick(
        &self,
        request: Picker,
        cx: &mut App,
    ) -> LocalBoxFuture<'static, Result<Option<PathBuf>>> {
        if request.initial_directory.is_some() || !request.filters.is_empty() {
            return Box::pin(async {
                Err(ServiceError::Unsupported("GPUI's picker cannot express filters or an initial directory; use a native dialog adapter".into()))
            });
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: !request.directory,
            directories: request.directory,
            multiple: false,
            prompt: Some(request.prompt.into()),
        });
        Box::pin(async move {
            let result = receiver
                .await
                .map_err(|_| ServiceError::Closed)?
                .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
            Ok(result.and_then(|paths| paths.into_iter().next()))
        })
    }
}

#[derive(Clone, Debug)]
pub enum ConfirmationState {
    Open,
    Pending,
    Failed(ServiceError),
    Closed,
}

pub struct Confirmation {
    pub title: String,
    pub description: String,
    pub restore_focus: gpui::FocusHandle,
    pub state: ConfirmationState,
}

impl Confirmation {
    pub fn begin(&mut self) -> Result<()> {
        if !matches!(
            self.state,
            ConfirmationState::Open | ConfirmationState::Failed(_)
        ) {
            return Err(ServiceError::Busy);
        }
        self.state = ConfirmationState::Pending;
        Ok(())
    }

    pub fn complete(&mut self, result: Result<()>) {
        if matches!(self.state, ConfirmationState::Pending) {
            self.state = match result {
                Ok(()) => ConfirmationState::Closed,
                Err(error) => ConfirmationState::Failed(error),
            };
        }
    }

    pub fn dismiss(&mut self) -> bool {
        if matches!(self.state, ConfirmationState::Pending) {
            return false;
        }
        self.state = ConfirmationState::Closed;
        true
    }
}
