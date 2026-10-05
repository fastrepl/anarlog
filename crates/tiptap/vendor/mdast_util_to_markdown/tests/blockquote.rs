use markdown::mdast::{
    Blockquote, Break, Code, Definition, Emphasis, Heading, Html, Image, ImageReference,
    InlineCode, Link, LinkReference, List, ListItem, Node, Paragraph, ReferenceKind, Strong, Text,
    ThematicBreak,
};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn block_quote() {
    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![],
            position: None,
        }))
        .unwrap(),
        ">\n",
        "should support a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n",
        "should support a block quote w/ a child"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
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
            position: None,
        }))
        .unwrap(),
        "> a\n>\n> ***\n>\n> b\n",
        "should support a block quote w/ children"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a\nb"),
                    position: None
                })],
                position: None
            }),],
            position: None,
        }))
        .unwrap(),
        "> a\n> b\n",
        "should support text w/ a line ending in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    }),
                    Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })
                ],
                position: None
            }),],
            position: None,
        }))
        .unwrap(),
        "> ab\n",
        "should support adjacent texts in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a\nb"),
                        position: None
                    })],
                    position: None
                }),
                Node::Blockquote(Blockquote {
                    children: vec![
                        Node::Paragraph(Paragraph {
                            children: vec![
                                Node::Text(Text {
                                    value: String::from("a\n"),
                                    position: None
                                }),
                                Node::InlineCode(InlineCode {
                                    value: String::from("b\nc"),
                                    position: None
                                }),
                                Node::Text(Text {
                                    value: String::from("\nd"),
                                    position: None
                                }),
                            ],
                            position: None
                        }),
                        Node::Heading(Heading {
                            children: vec![Node::Text(Text {
                                value: String::from("a b"),
                                position: None
                            })],
                            position: None,
                            depth: 1
                        })
                    ],
                    position: None
                }),
            ],
            position: None,
        }))
        .unwrap(),
        "> a\n> b\n>\n> > a\n> > `b\n> > c`\n> > d\n> >\n> > # a b\n",
        "should support a block quote in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    }),
                    Node::Break(Break { position: None }),
                    Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })
                ],
                position: None
            }),],
            position: None,
        }))
        .unwrap(),
        "> a\\\n> b\n",
        "should support a break in a block quote"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Blockquote(Blockquote {
                children: vec![Node::Code(Code {
                    value: String::from("a\nb\n\nc"),
                    position: None,
                    lang: None,
                    meta: None
                })],
                position: None,
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        ">     a\n>     b\n>\n>     c\n",
        "should support code (flow, indented) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Code(Code {
                value: String::from("c\nd\n\ne"),
                position: None,
                lang: String::from("a\nb").into(),
                meta: None
            })],
            position: None,
        }))
        .unwrap(),
        "> ```a&#xA;b\n> c\n> d\n>\n> e\n> ```\n",
        "should support code (flow, fenced) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::InlineCode(InlineCode {
                        value: String::from("b\nc"),
                        position: None
                    }),
                    Node::Text(Text {
                        value: String::from("\nd"),
                        position: None
                    })
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> `b\n> c`\n> d\n",
        "should support code (text) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![
                Node::Definition(Definition {
                    position: None,
                    title: Some("e\nf".into()),
                    url: "c\nd".into(),
                    identifier: "a\nb".into(),
                    label: None
                }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a\nb"),
                        position: None
                    })],
                    position: None
                })
            ],
            position: None,
        }))
        .unwrap(),
        "> [a\n> b]: <c&#xA;d> \"e\n> f\"\n>\n> a\n> b\n",
        "should support a definition in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::Emphasis(Emphasis {
                        children: vec![Node::Text(Text {
                            value: String::from("c\nd"),
                            position: None
                        }),],
                        position: None
                    }),
                    Node::Text(Text {
                        value: String::from("\nd"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> *c\n> d*\n> d\n",
        "should support an emphasis in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a\nb"),
                    position: None
                }),],
                position: None,
                depth: 3
            })],
            position: None,
        }))
        .unwrap(),
        "> ### a&#xA;b\n",
        "should support a heading (atx) in a block quote"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Blockquote(Blockquote {
                children: vec![Node::Heading(Heading {
                    children: vec![Node::Text(Text {
                        value: String::from("a\nb"),
                        position: None
                    }),],
                    position: None,
                    depth: 1
                })],
                position: None,
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "> a\n> b\n> =\n",
        "should support a heading (setext) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Html(Html {
                value: String::from("<div\nhidden>"),
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> <div\n> hidden>\n",
        "should support html (flow) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    }),
                    Node::Html(Html {
                        value: String::from("<span\nhidden>"),
                        position: None
                    }),
                    Node::Text(Text {
                        value: String::from("\nb"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a <span\n> hidden>\n> b\n",
        "should support html (text) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::Image(Image {
                        position: None,
                        alt: String::from("d\ne"),
                        url: String::from("b\nc"),
                        title: Some(String::from("f\ng"))
                    }),
                    Node::Text(Text {
                        value: String::from("\nh"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> ![d\n> e](<b&#xA;c> \"f\n> g\")\n> h\n",
        "should support an image (resource) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::ImageReference(ImageReference {
                        position: None,
                        alt: String::from("b\nc"),
                        label: Some(String::from("d\ne")),
                        reference_kind: ReferenceKind::Collapsed,
                        identifier: String::from("f"),
                    }),
                    Node::Text(Text {
                        value: String::from("\ng"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> ![b\n> c][d\n> e]\n> g\n",
        "should support an image (reference) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::Link(Link {
                        children: vec![Node::Text(Text {
                            value: String::from("d\ne"),
                            position: None
                        })],
                        position: None,
                        url: String::from("b\nc"),
                        title: Some(String::from("f\ng"))
                    }),
                    Node::Text(Text {
                        value: String::from("\nh"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> [d\n> e](<b&#xA;c> \"f\n> g\")\n> h\n",
        "should support a link (resource) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::LinkReference(LinkReference {
                        children: vec![Node::Text(Text {
                            value: String::from("b\nc"),
                            position: None
                        }),],
                        position: None,
                        reference_kind: ReferenceKind::Collapsed,
                        identifier: String::from("f"),
                        label: Some(String::from("d\ne"))
                    }),
                    Node::Text(Text {
                        value: String::from("\ng"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> [b\n> c][d\n> e]\n> g\n",
        "should support a link (reference) in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a\nb"),
                        position: None
                    })],
                    position: None
                }),
                Node::List(List {
                    children: vec![
                        Node::ListItem(ListItem {
                            children: vec![Node::Paragraph(Paragraph {
                                children: vec![Node::Text(Text {
                                    value: String::from("c\nd"),
                                    position: None
                                })],
                                position: None
                            })],
                            position: None,
                            spread: false,
                            checked: None
                        }),
                        Node::ListItem(ListItem {
                            children: vec![Node::ThematicBreak(ThematicBreak { position: None })],
                            position: None,
                            spread: false,
                            checked: None
                        }),
                        Node::ListItem(ListItem {
                            children: vec![Node::Paragraph(Paragraph {
                                children: vec![Node::Text(Text {
                                    value: String::from("e\nf"),
                                    position: None
                                })],
                                position: None
                            })],
                            position: None,
                            spread: false,
                            checked: None
                        }),
                    ],
                    position: None,
                    ordered: false,
                    start: None,
                    spread: false
                })
            ],
            position: None,
        }))
        .unwrap(),
        "> a\n> b\n>\n> - c\n>   d\n> - ***\n> - e\n>   f\n",
        "should support a list in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: String::from("a\n"),
                        position: None
                    }),
                    Node::Strong(Strong {
                        children: vec![Node::Text(Text {
                            value: String::from("c\nd"),
                            position: None
                        })],
                        position: None
                    }),
                    Node::Text(Text {
                        value: String::from("\nd"),
                        position: None
                    }),
                ],
                position: None
            })],
            position: None,
        }))
        .unwrap(),
        "> a\n> **c\n> d**\n> d\n",
        "should support a strong in a block quote"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![
                Node::ThematicBreak(ThematicBreak { position: None }),
                Node::ThematicBreak(ThematicBreak { position: None })
            ],
            position: None,
        }))
        .unwrap(),
        "> ***\n>\n> ***\n",
        "should support a thematic break in a block quote"
    );
}
