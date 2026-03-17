use std::fmt::Write;

use clap::Command;

pub fn generate(cmd: &Command) -> String {
    let mut cmd = cmd.clone();
    cmd.build();

    let mut out = String::new();
    render_command(&mut out, &cmd, cmd.get_name().to_string());
    out
}

fn render_command(out: &mut String, cmd: &Command, full_name: String) {
    let depth = full_name.matches(' ').count();
    let heading = if depth == 0 { "##" } else { "###" };

    writeln!(out, "{heading} {full_name}\n").unwrap();

    if let Some(about) = cmd.get_long_about().or_else(|| cmd.get_about()) {
        writeln!(out, "{about}\n").unwrap();
    }

    render_synopsis(out, cmd, &full_name, depth == 0);
    render_options(out, cmd, depth == 0);
    render_subcommands(out, cmd, &full_name);
}

fn is_flag(arg: &clap::Arg) -> bool {
    matches!(
        arg.get_action(),
        clap::ArgAction::SetTrue | clap::ArgAction::SetFalse | clap::ArgAction::Count
    )
}

fn render_synopsis(out: &mut String, cmd: &Command, full_name: &str, is_root: bool) {
    write!(out, "```\n{full_name}").unwrap();

    for arg in sorted_args(cmd) {
        if arg.is_positional() || arg.is_hide_set() {
            continue;
        }
        if !is_root && arg.is_global_set() {
            continue;
        }
        let (open, close) = if arg.is_required_set() {
            ("<", ">")
        } else {
            ("[", "]")
        };
        write!(out, " {open}").unwrap();
        if let Some(short) = arg.get_short() {
            write!(out, "-{short}").unwrap();
        } else if let Some(long) = arg.get_long() {
            write!(out, "--{long}").unwrap();
        }
        write!(out, "{close}").unwrap();
    }

    for arg in sorted_args(cmd) {
        if !arg.is_positional() || arg.is_hide_set() {
            continue;
        }
        let (open, close) = if arg.is_required_set() {
            ("<", ">")
        } else {
            ("[", "]")
        };
        write!(out, " {open}{}{close}", arg.get_id()).unwrap();
    }

    if cmd.has_subcommands() {
        write!(out, " <command>").unwrap();
    }

    writeln!(out, "\n```\n").unwrap();
}

fn render_options(out: &mut String, cmd: &Command, is_root: bool) {
    let locals: Vec<_> = sorted_args(cmd)
        .filter(|a| !a.is_positional() && !a.is_hide_set() && !a.is_global_set())
        .collect();
    let positional: Vec<_> = sorted_args(cmd)
        .filter(|a| a.is_positional() && !a.is_hide_set())
        .collect();

    if is_root {
        let globals: Vec<_> = sorted_args(cmd)
            .filter(|a| !a.is_positional() && !a.is_hide_set() && a.is_global_set())
            .collect();

        if !globals.is_empty() {
            writeln!(out, "**Global options:**\n").unwrap();
            for arg in &globals {
                render_arg(out, arg);
            }
            writeln!(out).unwrap();
        }
    }

    if !locals.is_empty() {
        writeln!(out, "**Options:**\n").unwrap();
        for arg in &locals {
            render_arg(out, arg);
        }
        writeln!(out).unwrap();
    }

    if !positional.is_empty() {
        writeln!(out, "**Arguments:**\n").unwrap();
        for arg in &positional {
            render_positional(out, arg);
        }
        writeln!(out).unwrap();
    }
}

fn render_arg(out: &mut String, arg: &clap::Arg) {
    let mut flags = Vec::new();
    if let Some(short) = arg.get_short() {
        flags.push(format!("-{short}"));
    }
    if let Some(long) = arg.get_long() {
        flags.push(format!("--{long}"));
    }

    let value = if is_flag(arg) {
        String::new()
    } else {
        arg.get_value_names()
            .map(|names| names.iter().map(|n| format!(" <{n}>")).collect::<String>())
            .unwrap_or_default()
    };

    write!(out, "- `{}{value}`", flags.join(", ")).unwrap();
    if let Some(help) = arg.get_long_help().or_else(|| arg.get_help()) {
        write!(out, " — {help}").unwrap();
    }
    append_default(out, arg);
    writeln!(out).unwrap();
}

fn render_positional(out: &mut String, arg: &clap::Arg) {
    let name = arg
        .get_value_names()
        .and_then(|v| v.first().map(|s| s.to_string()))
        .unwrap_or_else(|| arg.get_id().to_string().to_uppercase());

    write!(out, "- `<{name}>`").unwrap();
    if let Some(help) = arg.get_long_help().or_else(|| arg.get_help()) {
        write!(out, " — {help}").unwrap();
    }
    append_default(out, arg);
    writeln!(out).unwrap();
}

fn append_default(out: &mut String, arg: &clap::Arg) {
    if arg.is_hide_default_value_set() || is_flag(arg) {
        return;
    }
    let defaults: Vec<_> = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy())
        .collect();
    if !defaults.is_empty() {
        write!(out, " (default: `{}`)", defaults.join(", ")).unwrap();
    }
}

fn render_subcommands(out: &mut String, cmd: &Command, full_name: &str) {
    let subs: Vec<_> = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .collect();

    if subs.is_empty() {
        return;
    }

    writeln!(out, "**Subcommands:**\n").unwrap();
    for sub in &subs {
        let name = sub.get_name();
        write!(out, "- `{name}`").unwrap();
        if let Some(about) = sub.get_about() {
            write!(out, " — {about}").unwrap();
        }
        writeln!(out).unwrap();
    }
    writeln!(out).unwrap();

    writeln!(out, "---\n").unwrap();

    for sub in &subs {
        let child_name = format!("{full_name} {}", sub.get_name());
        render_command(out, sub, child_name);
    }
}

fn sorted_args(cmd: &Command) -> impl Iterator<Item = &clap::Arg> {
    let mut args: Vec<_> = cmd.get_arguments().filter(|a| !a.is_hide_set()).collect();
    args.sort_by_key(|a| {
        let order = a.get_display_order();
        let key = if let Some(short) = a.get_short() {
            short.to_lowercase().to_string()
        } else if let Some(long) = a.get_long() {
            long.to_string()
        } else {
            a.get_id().to_string()
        };
        (order, key)
    });
    args.into_iter()
}
