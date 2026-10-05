use markdown::mdast::{
    Break, Code, Definition, Heading, List, ListItem, Node, Paragraph, Root, Text, ThematicBreak,
};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn core() {
    assert_eq!(
        to(&Node::Root(Root {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::ThematicBreak(ThematicBreak { position: None }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })],
                    position: None
                }),
            ],
            position: None
        }))
        .unwrap(),
        "a\n\n***\n\nb\n",
        "should support root"
    );

    assert_eq!(
        to(&Node::Root(Root {
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
            position: None
        }))
        .unwrap(),
        "a\\\nb\n",
        "should not use blank lines between nodes when given phrasing"
    );

    assert_eq!(
        to(&Node::Root(Root {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::Definition(Definition {
                    position: None,
                    url: String::new(),
                    title: None,
                    identifier: String::from("b"),
                    label: None
                }),
                Node::Definition(Definition {
                    position: None,
                    url: String::new(),
                    title: None,
                    identifier: String::from("c"),
                    label: None
                }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("d"),
                        position: None
                    })],
                    position: None
                }),
            ],
            position: None
        }))
        .unwrap(),
        "a\n\n[b]: <>\n\n[c]: <>\n\nd\n",
        "should support adjacent definitions"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Root(Root {
                children: vec![
                    Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("a"),
                            position: None
                        })],
                        position: None
                    }),
                    Node::Definition(Definition {
                        position: None,
                        url: String::new(),
                        title: None,
                        identifier: String::from("b"),
                        label: None
                    }),
                    Node::Definition(Definition {
                        position: None,
                        url: String::new(),
                        title: None,
                        identifier: String::from("c"),
                        label: None
                    }),
                    Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("d"),
                            position: None
                        })],
                        position: None
                    }),
                ],
                position: None
            }),
            &Options {
                tight_definitions: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "a\n\n[b]: <>\n[c]: <>\n\nd\n",
        "should support tight adjacent definitions when `tight_definitions: true`"
    );

    assert_eq!(
        to(&Node::Root(Root {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::List(List {
                    children: vec![Node::ListItem(ListItem {
                        children: vec![],
                        position: None,
                        spread: false,
                        checked: None
                    })],
                    position: None,
                    ordered: false,
                    start: None,
                    spread: false
                }),
                Node::List(List {
                    children: vec![Node::ListItem(ListItem {
                        children: vec![],
                        position: None,
                        spread: false,
                        checked: None
                    })],
                    position: None,
                    ordered: false,
                    start: None,
                    spread: false
                }),
                Node::List(List {
                    children: vec![Node::ListItem(ListItem {
                        children: vec![],
                        position: None,
                        spread: false,
                        checked: None
                    })],
                    position: None,
                    ordered: true,
                    start: None,
                    spread: false
                }),
                Node::List(List {
                    children: vec![Node::ListItem(ListItem {
                        children: vec![],
                        position: None,
                        spread: false,
                        checked: None
                    })],
                    position: None,
                    ordered: true,
                    start: None,
                    spread: false
                }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("d"),
                        position: None
                    })],
                    position: None
                }),
            ],
            position: None
        }))
        .unwrap(),
        "a\n\n*\n\n-\n\n1.\n\n1)\n\nd\n",
        "should use a different marker for adjacent lists"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Root(Root {
                children: vec![
                    Node::Code(Code {
                        value: String::from("a"),
                        position: None,
                        lang: None,
                        meta: None
                    }),
                    Node::List(List {
                        children: vec![Node::ListItem(ListItem {
                            children: vec![],
                            position: None,
                            spread: false,
                            checked: None
                        })],
                        position: None,
                        ordered: false,
                        start: None,
                        spread: false
                    }),
                    Node::Code(Code {
                        value: String::from("b"),
                        position: None,
                        lang: None,
                        meta: None
                    }),
                ],
                position: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "    a\n\n*\n\n<!---->\n\n    b\n",
        "should inject HTML comments between lists and an indented code"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Root(Root {
                children: vec![
                    Node::Code(Code {
                        value: String::from("a"),
                        position: None,
                        lang: None,
                        meta: None
                    }),
                    Node::Code(Code {
                        value: String::from("b"),
                        position: None,
                        lang: None,
                        meta: None
                    }),
                ],
                position: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "    a\n\n<!---->\n\n    b\n",
        "should inject HTML comments between adjacent indented code"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })],
                    position: None
                }),
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n\n  b\n",
        "should not honour `spread: false` for two paragraphs"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::Definition(Definition {
                    position: None,
                    url: String::from("d"),
                    title: None,
                    identifier: String::from("b"),
                    label: Some(String::from("c"))
                }),
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n\n  [c]: d\n",
        "should not honour `spread: false` for a paragraph and a definition"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::Heading(Heading {
                    children: vec![Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })],
                    position: None,
                    depth: 1
                })
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n  # b\n",
        "should honour `spread: false` for a paragraph and a heading"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![
                    Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("a"),
                            position: None
                        })],
                        position: None
                    }),
                    Node::Heading(Heading {
                        children: vec![Node::Text(Text {
                            value: String::from("b"),
                            position: None
                        })],
                        position: None,
                        depth: 1
                    })
                ],
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "* a\n\n  b\n  =\n",
        "should not honour `spread: false` for a paragraph and a setext heading"
    );
}
