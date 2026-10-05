use markdown::mdast::{Code, Node};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn text() {
    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                value: String::from("a"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "    a\n",
        "should support code w/ a value (indent)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("a"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n```\n",
        "should support code w/ a value (fences)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\n```\n",
        "should support code w/ a lang"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: None,
            meta: Some("a".to_string())
        }))
        .unwrap(),
        "```\n```\n",
        "should support (ignore) code w/ only a meta"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a".to_string()),
            meta: Some("b".to_string())
        }))
        .unwrap(),
        "```a b\n```\n",
        "should support code w/ lang and meta"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a&#x20;b\n```\n",
        "should encode a space in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a\nb".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a&#xA;b\n```\n",
        "should encode a line ending in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a`b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a&#x60;b\n```\n",
        "should encode a grave accent in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("a\\-b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\\\\-b\n```\n",
        "should escape a backslash in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a b".to_string())
        }))
        .unwrap(),
        "```x a b\n```\n",
        "should not encode a space in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a\nb".to_string())
        }))
        .unwrap(),
        "```x a&#xA;b\n```\n",
        "should encode a line ending in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a`b".to_string())
        }))
        .unwrap(),
        "```x a&#x60;b\n```\n",
        "should encode a grave accent in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a\\-b".to_string())
        }))
        .unwrap(),
        "```x a\\\\-b\n```\n",
        "should escape a backslash in `meta`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                value: String::new(),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~\n~~~\n",
        "should support fenced code w/ tildes when `fence: \"~\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                value: String::new(),
                position: None,
                lang: Some("a`b".to_string()),
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~a`b\n~~~\n",
        "should not encode a grave accent when using tildes for fences"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("```\nasd\n```"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "````\n```\nasd\n```\n````\n",
        "should use more grave accents for fences if there are streaks of grave accents in the value (fences)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                value: String::from("~~~\nasd\n~~~"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~~\n~~~\nasd\n~~~\n~~~~\n",
        "should use more tildes for fences if there are streaks of tildes in the value (fences)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("b"),
            position: None,
            lang: Some("a".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\nb\n```\n",
        "should use a fence if there is an info"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from(" "),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n \n```\n",
        "should use a fence if there is only whitespace"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("\na"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n\na\n```\n",
        "should use a fence if there first line is blank (void)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from(" \na"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n \na\n```\n",
        "should use a fence if there first line is blank (filled)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("a\n"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n\n```\n",
        "should use a fence if there last line is blank (void)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            value: String::from("a\n "),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n \n```\n",
        "should use a fence if there last line is blank (filled)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                value: String::from("  a\n\n b"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "      a\n\n     b\n",
        "should use an indent if the value is indented"
    );
}
