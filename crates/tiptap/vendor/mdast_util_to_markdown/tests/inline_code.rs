use markdown::mdast::{InlineCode, Node};
use mdast_util_to_markdown::to_markdown as to;
use pretty_assertions::assert_eq;

#[test]
fn text() {
    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::new(),
            position: None
        }))
        .unwrap(),
        "``\n",
        "should support an empty code text"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a"),
            position: None
        }))
        .unwrap(),
        "`a`\n",
        "should support a code text"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" "),
            position: None
        }))
        .unwrap(),
        "` `\n",
        "should support a space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("\n"),
            position: None
        }))
        .unwrap(),
        "`\n`\n",
        "should support an eol"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("  "),
            position: None
        }))
        .unwrap(),
        "`  `\n",
        "should support several spaces"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a`b"),
            position: None
        }))
        .unwrap(),
        "``a`b``\n",
        "should use a fence of two grave accents if the value contains one"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a``b"),
            position: None
        }))
        .unwrap(),
        "`a``b`\n",
        "should use a fence of one grave accent if the value contains two"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a``b`c"),
            position: None
        }))
        .unwrap(),
        "```a``b`c```\n",
        "should use a fence of three grave accents if the value contains two and one"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("`a"),
            position: None
        }))
        .unwrap(),
        "`` `a ``\n",
        "should pad w/ a space if the value starts w/ a grave accent"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a`"),
            position: None
        }))
        .unwrap(),
        "`` a` ``\n",
        "should pad w/ a space if the value ends w/ a grave accent"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" a "),
            position: None
        }))
        .unwrap(),
        "`  a  `\n",
        "should pad w/ a space if the value starts and ends w/ a space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" a"),
            position: None
        }))
        .unwrap(),
        "` a`\n",
        "should not pad w/ spaces if the value ends w/ a non-space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a "),
            position: None
        }))
        .unwrap(),
        "`a `\n",
        "should not pad w/ spaces if the value starts w/ a non-space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n- b"),
            position: None
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (-)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n#"),
            position: None
        }))
        .unwrap(),
        "`a #`\n",
        "should prevent breaking out of code (#)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n1. "),
            position: None
        }))
        .unwrap(),
        "`a 1. `\n",
        "should prevent breaking out of code (\\d\\.)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\r- b"),
            position: None
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (cr)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\r\n- b"),
            position: None
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (crlf)"
    );
}
