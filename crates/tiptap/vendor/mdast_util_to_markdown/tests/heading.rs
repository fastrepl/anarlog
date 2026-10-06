use markdown::mdast::{Break, Heading, Html, Node, Text};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn heading() {
    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "#\n",
        "should serialize a heading w/ rank 1"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![],
            position: None,
            depth: 6
        }))
        .unwrap(),
        "######\n",
        "should serialize a heading w/ rank 6"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![],
            position: None,
            depth: 7
        }))
        .unwrap(),
        "######\n",
        "should serialize a heading w/ rank 7 as 6"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![],
            position: None,
            depth: 0
        }))
        .unwrap(),
        "#\n",
        "should serialize a heading w/ rank 0 as 1"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a\n",
        "should serialize a heading w/ content"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None,
                depth: 1
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "a\n=\n",
        "should serialize a heading w/ rank 1 as setext when `setext: true`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None,
                depth: 2
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "a\n-\n",
        "should serialize a heading w/ rank 2 as setext when `setext: true`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None,
                depth: 3
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "### a\n",
        "should serialize a heading w/ rank 3 as atx when `setext: true`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("aa\rb"),
                    position: None
                })],
                position: None,
                depth: 2
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "aa\rb\n-\n",
        "should serialize a setext underline as long as the last line (1)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a\r\nbbb"),
                    position: None
                })],
                position: None,
                depth: 1
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "a\r\nbbb\n===\n",
        "should serialize a setext underline as long as the last line (2)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![],
                position: None,
                depth: 1
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "#\n",
        "should serialize an empty heading w/ rank 1 as atx when `setext: true`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![],
                position: None,
                depth: 2
            }),
            &Options {
                setext: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "##\n",
        "should serialize an empty heading w/ rank 1 as atx when `setext: true`"
    );

    //assert_eq!(
    //    to(&Node::Heading(Heading {
    //        children: vec![],
    //        position: None,
    //        depth: 1
    //    }),)
    //    .unwrap(),
    //    "`\n`\n=\n",
    //    "should serialize an heading w/ rank 1 and code w/ a line ending as setext"
    //);

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Html(Html {
                value: "<a\n/>".to_string(),
                position: None
            })],
            position: None,
            depth: 1
        }),)
        .unwrap(),
        "<a\n/>\n==\n",
        "should serialize an heading w/ rank 1 and html w/ a line ending as setext"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a\nb"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "a\nb\n=\n",
        "should serialize an heading w/ rank 1 and text w/ a line ending as setext"
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
            depth: 1
        }))
        .unwrap(),
        "a\\\nb\n=\n",
        "should serialize an heading w/ rank 1 and a break as setext"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![],
                position: None,
                depth: 1
            }),
            &Options {
                close_atx: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "# #\n",
        "should serialize a heading with a closing sequence when `closeAtx` (empty)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Heading(Heading {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None,
                depth: 3
            }),
            &Options {
                close_atx: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "### a ###\n",
        "should serialize a with a closing sequence when `closeAtx` (content)"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("# a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## # a\n",
        "should not escape a `#` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("1) a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## 1) a\n",
        "should not escape a `1)` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("+ a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## + a\n",
        "should not escape a `+` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("- a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## - a\n",
        "should not escape a `-` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("= a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## = a\n",
        "should not escape a `=` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("> a"),
                position: None
            })],
            position: None,
            depth: 2
        }))
        .unwrap(),
        "## > a\n",
        "should not escape a `>` at the start of phrasing in a heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a #"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a \\#\n",
        "should escape a `#` at the end of a heading (1)"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a ##"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a #\\#\n",
        "should escape a `#` at the end of a heading (2)"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a # b"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a # b\n",
        "should not escape a `#` in a heading (2)"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("  a"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# &#x20; a\n",
        "should encode a space at the start of an atx heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("\t\ta"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# &#x9;\ta\n",
        "should encode a tab at the start of an atx heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a  "),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a &#x20;\n",
        "should encode a space at the end of an atx heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a\t\t"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "# a\t&#x9;\n",
        "should encode a tab at the end of an atx heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a \n b"),
                position: None
            })],
            position: None,
            depth: 1
        }))
        .unwrap(),
        "a&#x20;\n&#x20;b\n=======\n",
        "should encode spaces around a line ending in a setext heading"
    );

    assert_eq!(
        to(&Node::Heading(Heading {
            children: vec![Node::Text(Text {
                value: String::from("a \n b"),
                position: None
            })],
            position: None,
            depth: 3
        }))
        .unwrap(),
        "### a &#xA; b\n",
        "should not need to encode spaces around a line ending in an atx heading (because the line ending is encoded)"
    );
}
