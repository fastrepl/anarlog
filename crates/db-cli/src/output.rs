pub fn print_json(value: &impl serde::Serialize) -> crate::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| crate::Error::operation_failed("serialize response", e.to_string()))?;
    println!("{}", String::from_utf8_lossy(&bytes));
    Ok(())
}
