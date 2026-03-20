use std::borrow::Cow;

use crate::Error;

pub(super) fn sanitise_key(key: &str) -> Cow<'_, str> {
    let valid = key
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && key
            .chars()
            .next()
            .map(|first| !first.is_numeric())
            .unwrap_or(true);

    if valid {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(format!("\"{}\"", escape_string(key)))
    }
}

pub(super) fn sanitise_type_name(ident: &str) -> Result<(), Error> {
    if let Some(first_char) = ident.chars().next()
        && !first_char.is_alphabetic()
        && first_char != '_'
    {
        return Err(Error::InvalidTypeName(ident.to_string()));
    }

    if ident.contains(|c: char| !c.is_alphanumeric() && c != '_') {
        return Err(Error::InvalidTypeName(ident.to_string()));
    }

    Ok(())
}

pub(super) fn escape_string(s: &str) -> Cow<'_, str> {
    if s.contains(['\\', '"', '\n', '\r', '\t']) {
        let mut result = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                _ => result.push(c),
            }
        }
        Cow::Owned(result)
    } else {
        Cow::Borrowed(s)
    }
}

pub(super) fn escape_char(c: char) -> Cow<'static, str> {
    match c {
        '\\' => Cow::Borrowed("\\\\"),
        '"' => Cow::Borrowed("\\\""),
        '\n' => Cow::Borrowed("\\n"),
        '\r' => Cow::Borrowed("\\r"),
        '\t' => Cow::Borrowed("\\t"),
        _ => Cow::Owned(c.to_string()),
    }
}

pub(super) fn to_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in name.chars().enumerate() {
        if c == '_' || c == '-' {
            capitalize_next = true;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}
