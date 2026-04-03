use std::io::{self, BufRead, Write};

use cactus::{CompleteOptions, Message, Model};

fn main() {
    let path = std::env::args().nth(1).expect("Usage: chat <model-path>");
    let mut model = Model::new(&path).expect("Failed to load model");

    let options = CompleteOptions {
        max_tokens: Some(1024),
        temperature: Some(0.7),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut messages: Vec<Message> = vec![Message::system("You are a helpful assistant.")];

    println!("Chat with your model. Type 'exit' to quit.\n");

    loop {
        print!("> ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() || input == "exit" || input == "quit" {
            break;
        }

        messages.push(Message::user(input));
        model.reset();

        let mut response_text = String::new();
        let result = model.complete_streaming(&messages, &options, |token| {
            print!("{token}");
            let _ = io::stdout().flush();
            response_text.push_str(token);
            true
        });

        println!();

        match result {
            Ok(_) => messages.push(Message::assistant(&response_text)),
            Err(e) => eprintln!("Error: {e}"),
        }
    }
}
