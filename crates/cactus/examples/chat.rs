/// cargo run -p cactus --features examples --example chat -- /path/to/model.gguf
use std::{
    error::Error,
    io::{self, BufRead, IsTerminal, Write},
};

use cactus::{CompleteOptions, Message, Model};
use colored::Colorize;

fn main() {
    if let Err(error) = run() {
        colored::control::set_override(io::stdout().is_terminal());
        eprintln!("{}: {error}", "error".red());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let model_path = std::env::args().nth(1).expect("usage: chat <model-path>");
    let mut model = Model::new(&model_path)?;
    let options = CompleteOptions {
        max_tokens: Some(1024),
        temperature: Some(0.7),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };
    let mut messages = vec![Message::system("You are a helpful assistant.")];
    colored::control::set_override(io::stdout().is_terminal());

    print_intro();

    loop {
        let Some(input) = read_user_message()? else {
            break;
        };

        match chat_turn(&mut model, &options, &mut messages, &input) {
            Ok(response) => messages.push(Message::assistant(&response)),
            Err(error) => eprintln!("{}: {error}", "error".red()),
        }
    }

    Ok(())
}

fn print_intro() {
    println!(
        "{} Type {} or {} to stop.\n",
        "Chat ready.".cyan(),
        "exit".yellow(),
        "quit".yellow()
    );
}

fn read_user_message() -> io::Result<Option<String>> {
    loop {
        print!("{} {}", "you".blue(), ">".dimmed());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim();

        if matches!(input, "exit" | "quit") {
            return Ok(None);
        }
        if !input.is_empty() {
            return Ok(Some(input.to_owned()));
        }
    }
}

fn chat_turn(
    model: &mut Model,
    options: &CompleteOptions,
    messages: &mut Vec<Message>,
    input: &str,
) -> Result<String, Box<dyn Error>> {
    messages.push(Message::user(input));
    model.reset();

    print!("{} ", "assistant".green());
    io::stdout().flush()?;

    let mut response = String::new();
    let result = model.complete_streaming(messages, options, |token| {
        print!("{token}");
        let _ = io::stdout().flush();
        response.push_str(token);
        true
    });

    println!("\n");
    result?;

    Ok(response)
}
