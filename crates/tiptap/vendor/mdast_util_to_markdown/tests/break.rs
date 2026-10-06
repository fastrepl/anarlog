use markdown::{
    mdast::{Break, Heading, Node, Text},
    to_mdast as from,
};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn r#break() {
    assert_eq!(
        to(&Node::Break(Break { position: None })).unwrap(),
        "\\\n",
        "should support a break"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![
                Node::Text(Text {
                    value: String::from("a"),
                    position: None
                }),
                Node::Break(Break { position: None }),
                Node::Text(Text {
                    value: String::from("b"),
                    position: None
                }),
            ],
            position: None,
            depth: 3
        }))
        .unwrap(),
        "### a b\n",
        "should serialize breaks in heading (atx) as a space"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![
                Node::Text(Text {
                    value: String::from("a "),
                    position: None
                }),
                Node::Break(Break { position: None }),
                Node::Text(Text {
                    value: String::from("b"),
                    position: None
                }),
            ],
            position: None,
            depth: 3
        }))
        .unwrap(),
        "### a b\n",
        "should serialize breaks in heading (atx) as a space"
    );

    assert_eq!(
        to_md_with_opts(
            &from("a  \nb\n=\n", &Default::default()).unwrap(),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "a\\\nb\n=\n",
        "should serialize breaks in heading (setext)"
    );
}
