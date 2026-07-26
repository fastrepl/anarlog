use minijinja::{ErrorKind, Value};

pub fn transcript(segments: Value) -> Result<String, minijinja::Error> {
    let mut output = String::new();

    for segment in segments.try_iter()? {
        let speaker_label = segment
            .get_attr("speaker_label")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "Unknown Speaker".to_string());

        let text = segment
            .get_attr("text")
            .map(|v| v.to_string())
            .unwrap_or_default();

        output.push_str(&format!("[{}]\n{}\n\n", speaker_label, text));
    }

    Ok(output)
}

pub fn url(v: Value) -> Result<String, minijinja::Error> {
    let url = v.as_str().unwrap_or_default();

    let html = reqwest::blocking::get(url)
        .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))?
        .text()
        .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))?;

    let md = htmd::convert(&html)
        .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))?;

    Ok(md)
}
