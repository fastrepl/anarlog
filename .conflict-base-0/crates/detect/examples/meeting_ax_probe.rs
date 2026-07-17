fn main() {
    let inspections = detect::inspect_meeting_accessibility();
    println!("{inspections:#?}");
}
