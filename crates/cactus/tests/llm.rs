use std::sync::atomic::{AtomicUsize, Ordering};

use cactus::{CompleteOptions, Message, Model};

fn llm_model() -> Model {
    let path = std::env::var("CACTUS_LLM_MODEL")
        .unwrap_or_else(|_| "/tmp/cactus-models/gemma-3-270m-it".into());
    Model::new(&path).unwrap()
}

// cargo test -p cactus --test llm test_complete -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete() {
    let model = llm_model();
    let messages = vec![
        Message::system("Answer in one word only."),
        Message::user("What is 2+2?"),
    ];
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let r = model.complete(&messages, &options).unwrap();

    assert!(r.total_tokens > 0);
    println!("response: {:?}", r.text);
}

// cargo test -p cactus --test llm test_complete_streaming -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming() {
    let model = llm_model();
    let messages = vec![
        Message::system("Answer in one word only."),
        Message::user("What is 2+2?"),
    ];
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let token_count = AtomicUsize::new(0);

    let r = model
        .complete_streaming(&messages, &options, |token| {
            assert!(!token.is_empty());
            token_count.fetch_add(1, Ordering::Relaxed);
            true
        })
        .unwrap();

    println!(
        "streamed {} tokens: {:?}",
        token_count.load(Ordering::Relaxed),
        r.text
    );
}

// cargo test -p cactus --test llm test_complete_streaming_early_stop -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming_early_stop() {
    let model = llm_model();
    let messages = vec![Message::user("Count from 1 to 100")];
    let options = CompleteOptions {
        max_tokens: Some(200),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let token_count = AtomicUsize::new(0);

    let _ = model.complete_streaming(&messages, &options, |_token| {
        let n = token_count.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= 3 {
            model.stop();
            return false;
        }
        true
    });

    let final_count = token_count.load(Ordering::Relaxed);
    assert!(
        final_count < 200,
        "should have stopped early, got {final_count} tokens"
    );
    println!("stopped after {final_count} tokens");
}

// cargo test -p cactus --test llm test_complete_multi_turn -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_multi_turn() {
    let mut model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(30),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let r1 = model
        .complete(&[Message::user("Say exactly: pineapple")], &options)
        .unwrap();

    model.reset();

    let r2 = model
        .complete(
            &[
                Message::user("Say exactly: pineapple"),
                Message::assistant(&r1.text),
                Message::user("What fruit did I just ask you to say?"),
            ],
            &options,
        )
        .unwrap();

    assert!(r1.total_tokens > 0);
    assert!(r2.total_tokens > 0);
    println!("turn1: {:?}", r1.text);
    println!("turn2: {:?}", r2.text);
}
