fn main() {
    match power::snapshot() {
        Ok(snapshot) => println!("{snapshot:#?}"),
        Err(e) => eprintln!("error: {e}"),
    }
}
