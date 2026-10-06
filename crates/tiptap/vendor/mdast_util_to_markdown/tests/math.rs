use markdown::mdast::{Definition, InlineMath, Math, Node, Paragraph, Text};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn math() {
    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a"),
            position: None
        }))
        .unwrap(),
        "$a$\n",
        "should serialize math (text)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::InlineMath(InlineMath {
                value: String::from("a"),
                position: None
            }),
            &Options {
                single_dollar_text_math: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "$$a$$\n",
        "should serialize math (text) with at least 2 dollars w/ `single_dollar_text_math: false`"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::new(),
            position: None
        }))
        .unwrap(),
        "$$\n",
        "should serialize math (text) w/o `value`"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a \\$ b"),
            position: None
        }))
        .unwrap(),
        "$$a \\$ b$$\n",
        "should serialize math (text) w/ two dollar signs when including a dollar"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a \\$"),
            position: None
        }))
        .unwrap(),
        "$$ a \\$ $$\n",
        "should serialize math (text) w/ padding when ending in a dollar sign"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("$ a"),
            position: None
        }))
        .unwrap(),
        "$$ $ a $$\n",
        "should serialize math (text) w/ padding when starting in a dollar sign"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from(" a "),
            position: None
        }))
        .unwrap(),
        "$  a  $\n",
        "should pad w/ a space if the value starts and ends w/ a space"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from(" a"),
            position: None
        }))
        .unwrap(),
        "$ a$\n",
        "should not pad w/ spaces if the value ends w/ a non-space"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a "),
            position: None
        }))
        .unwrap(),
        "$a $\n",
        "should not pad w/ spaces if the value starts w/ a non-space"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::from("a"),
            position: None,
            meta: None
        }))
        .unwrap(),
        "$$\na\n$$\n",
        "should serialize math (flow)"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::new(),
            position: None,
            meta: None
        }))
        .unwrap(),
        "$$\n$$\n",
        "should serialize math (flow) w/o `value`"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::new(),
            position: None,
            meta: String::from("a").into()
        }))
        .unwrap(),
        "$$a\n$$\n",
        "should serialize math (flow) w/ `meta`"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::from("$$"),
            position: None,
            meta: None
        }))
        .unwrap(),
        "$$$\n$$\n$$$\n",
        "should serialize math (flow) w/ more dollars than occur together in `value`"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a $ b"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a \\$ b\n",
        "should escape `$` in phrasing"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a $ b"),
                    position: None
                })],
                position: None
            }),
            &Options {
                single_dollar_text_math: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "a $ b\n",
        "should not escape a single dollar in phrasing w/ `single_dollar_text_math: false`'"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a $$ b"),
                    position: None
                })],
                position: None
            }),
            &Options {
                single_dollar_text_math: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "a \\$$ b\n",
        "should escape two dollars in phrasing w/ `single_dollar_text_math: false`"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: String::from("a $"),
                    position: None
                }),
                Node::InlineMath(InlineMath {
                    value: String::from("b"),
                    position: None
                }),
                Node::Text(Text {
                    value: String::from("$ c"),
                    position: None
                }),
            ],
            position: None
        }))
        .unwrap(),
        "a \\$$b$\\$ c\n",
        "should escape `$` around math (text)"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            position: None,
            url: String::from("b"),
            title: String::from("a\n$\nb").into(),
            identifier: String::from("a"),
            label: String::from("a").into(),
        }))
        .unwrap(),
        "[a]: b \"a\n$\nb\"\n",
        "should not escape `$` at the start of a line"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::new(),
            position: None,
            meta: String::from("a\rb\nc").into()
        }))
        .unwrap(),
        "$$a&#xD;b&#xA;c\n$$\n",
        "should escape `\\r`, `\\n` when in `meta` of math (flow)"
    );

    assert_eq!(
        to(&Node::Math(Math {
            value: String::new(),
            position: None,
            meta: String::from("a$b").into()
        }))
        .unwrap(),
        "$$a&#x24;b\n$$\n",
        "should escape `$` when in `meta` of math (flow)"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a\n- b"),
            position: None
        }))
        .unwrap(),
        "$a - b$\n",
        "should prevent breaking out of code (-)"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a\n#"),
            position: None
        }))
        .unwrap(),
        "$a #$\n",
        "should prevent breaking out of code (#)"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a\n1. "),
            position: None
        }))
        .unwrap(),
        "$a 1. $\n",
        "should prevent breaking out of code (\\d\\.)"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a\r- b"),
            position: None
        }))
        .unwrap(),
        "$a - b$\n",
        "should prevent breaking out of code (cr)"
    );

    assert_eq!(
        to(&Node::InlineMath(InlineMath {
            value: String::from("a\n- b"),
            position: None
        }))
        .unwrap(),
        "$a - b$\n",
        "should prevent breaking out of code (crlf)"
    );
}
