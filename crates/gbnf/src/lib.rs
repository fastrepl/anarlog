// https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md

#[derive(specta::Type, serde::Serialize, serde::Deserialize)]
#[serde(tag = "task")]
pub enum Grammar {
    #[serde(rename = "enhance")]
    Enhance { sections: Option<Vec<String>> },
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "tags")]
    Tags,
    #[serde(rename = "email-to-name")]
    EmailToName,
}

impl Grammar {
    pub fn build(&self) -> String {
        match self {
            Grammar::Enhance { sections } => build_enhance_other_grammar(sections),
            Grammar::Title => build_title_grammar(),
            Grammar::Tags => build_tags_grammar(),
            Grammar::EmailToName => build_email_to_name_grammar(),
        }
    }
}

fn build_known_sections_grammar(sections: &[String]) -> String {
    let mut rules = vec![];

    let mut root_parts = vec![];
    for i in 0..sections.len() {
        root_parts.push(format!("section{}", i));
    }

    let root_rule = format!("root ::= {}", root_parts.join(" "));
    rules.push(root_rule);

    for (i, section) in sections.iter().enumerate() {
        let section_rule = format!(
            r##"section{} ::= "# {}\n\n" bline bline bline? bline? bline? "\n""##,
            i, section
        );
        rules.push(section_rule);
    }

    rules
        .push(r##"bline ::= "- **" [A-Z] [^*\n:]+ "**: " ([^*;,[.\n] | link)+ ".\n""##.to_string());
    rules.push(r##"link ::= "[" [^\]]+ "]" "(" [^)]+ ")""##.to_string());

    rules.join("\n")
}

fn build_enhance_other_grammar(s: &Option<Vec<String>>) -> String {
    let auto = [
        r##"root ::= thinking section section section section? section?"##,
        r##"section ::= header "\n\n" bline bline bline? bline? bline? "\n""##,
        r##"header ::= "# " [A-Z][^*.\n]+"##,
        r##"line ::= "- " [A-Z] [^*.\n[(]+ ".\n""##,
        r##"bline ::= "- **" [A-Z] [^*\n:]+ "**: " ([^*;,[.\n] | link)+ ".\n""##,
        r##"hd ::= "- " [A-Z] [^[(*\n]+ "\n""##,
        r##"thinking ::= "<thinking>\n" hd hd hd? hd? hd? "</thinking>""##,
        r##"link ::= "[" [^\]]+ "]" "(" [^)]+ ")""##,
    ]
    .join("\n");

    match s {
        None => auto,
        Some(v) if v.is_empty() => auto,
        Some(v) => build_known_sections_grammar(v),
    }
}

fn build_title_grammar() -> String {
    [
        r##"lowercase ::= [a-z]"##,
        r##"uppercase ::= [A-Z]"##,
        r##"number ::= [0-9]"##,
        r##"acronym ::= uppercase{2,}"##,
        r##"word ::= acronym | (uppercase | number) (lowercase | number)*"##,
        r##"root ::= word (" " word)*"##,
    ]
    .join("\n")
}

fn build_tags_grammar() -> String {
    [
        r##"root ::= "[" string ("," string ("," string ("," string)?)?)? "]""##,
        r##"string ::= "\"" tag "\"""##,
        r##"tag ::= [a-zA-Z] ([a-zA-Z0-9_-])*"##,
    ]
    .join("\n")
}

fn build_email_to_name_grammar() -> String {
    [r##"root ::= "{" ws "\"first_name\"" ws ":" ws string "," ws "\"last_name\"" ws ":" ws string "}" ws"##,
        r##"string ::= "\"" [^"\n]* "\"" ws"##,
        r##"ws ::= [ \t\n]*"##]
    .join("\n")
}
