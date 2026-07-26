use std::ffi::OsString;

use crate::naming::cli_flag;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum HookEvent {
    #[serde(rename = "afterListeningStopped")]
    #[specta(rename = "afterListeningStopped")]
    AfterListeningStopped { args: AfterListeningStoppedArgs },
    #[serde(rename = "beforeListeningStarted")]
    #[specta(rename = "beforeListeningStarted")]
    BeforeListeningStarted { args: BeforeListeningStartedArgs },
}

impl HookEvent {
    pub fn condition_key(&self) -> &'static str {
        match self {
            HookEvent::AfterListeningStopped { .. } => "afterListeningStopped",
            HookEvent::BeforeListeningStarted { .. } => "beforeListeningStarted",
        }
    }

    pub fn cli_args(&self) -> Vec<OsString> {
        match self {
            HookEvent::AfterListeningStopped { args } => args.to_cli_args(),
            HookEvent::BeforeListeningStarted { args } => args.to_cli_args(),
        }
    }
}

pub trait HookArgs {
    fn to_cli_args(&self) -> Vec<OsString>;
}

fn push_cli_arg(args: &mut Vec<OsString>, field_name: &str, value: &str) {
    args.push(OsString::from(cli_flag(field_name)));
    args.push(OsString::from(value));
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AfterListeningStoppedArgs {
    pub resource_dir: String,
    pub app_hyprnote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_meeting: Option<String>,
}

impl HookArgs for AfterListeningStoppedArgs {
    fn to_cli_args(&self) -> Vec<OsString> {
        let mut args = Vec::with_capacity(6);
        push_cli_arg(&mut args, stringify!(resource_dir), &self.resource_dir);
        push_cli_arg(&mut args, stringify!(app_hyprnote), &self.app_hyprnote);

        if let Some(meeting) = &self.app_meeting {
            push_cli_arg(&mut args, stringify!(app_meeting), meeting);
        }

        args
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BeforeListeningStartedArgs {
    pub resource_dir: String,
    pub app_hyprnote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_meeting: Option<String>,
}

impl HookArgs for BeforeListeningStartedArgs {
    fn to_cli_args(&self) -> Vec<OsString> {
        let mut args = Vec::with_capacity(6);
        push_cli_arg(&mut args, stringify!(resource_dir), &self.resource_dir);
        push_cli_arg(&mut args, stringify!(app_hyprnote), &self.app_hyprnote);

        if let Some(meeting) = &self.app_meeting {
            push_cli_arg(&mut args, stringify!(app_meeting), meeting);
        }

        args
    }
}
