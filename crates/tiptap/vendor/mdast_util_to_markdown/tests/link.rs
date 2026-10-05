use markdown::mdast::{Link, Node, Text};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn text() {
    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "[]()\n",
        "should support a link"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "[a]()\n",
        "should support children"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("a"),
            title: None
        }))
        .unwrap(),
        "[](a)\n",
        "should support a url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: Some(String::from("a"))
        }))
        .unwrap(),
        "[](<> \"a\")\n",
        "should support a title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("a"),
            title: Some(String::from("b"))
        }))
        .unwrap(),
        "[](a \"b\")\n",
        "should support a url and title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b c"),
            title: None
        }))
        .unwrap(),
        "[](<b c>)\n",
        "should support a link w/ enclosed url w/ whitespace in url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b <c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\<c>)\n",
        "should escape an opening angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b >c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\>c>)\n",
        "should escape a closing angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b \\+c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\\\+c>)\n",
        "should escape a backslash in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b\nc"),
            title: None
        }))
        .unwrap(),
        "[](<b&#xA;c>)\n",
        "should encode a line ending in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b(c"),
            title: None
        }))
        .unwrap(),
        "[](b\\(c)\n",
        "should escape an opening paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b)c"),
            title: None
        }))
        .unwrap(),
        "[](b\\)c)\n",
        "should escape a closing paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b\\.c"),
            title: None
        }))
        .unwrap(),
        "[](b\\\\.c)\n",
        "should escape a backslash in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("\x0C"),
            title: None
        }))
        .unwrap(),
        "[](<\x0C>)\n",
        "should support control characters in links"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: Some(String::from("b\\-c"))
        }))
        .unwrap(),
        "[](<> \"b\\\\-c\")\n",
        "should escape a backslash in `title`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: None
        }))
        .unwrap(),
        "<tel:123>\n",
        "should use an autolink for nodes w/ a value similar to the url and a protocol"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: vec![Node::Text(Text {
                    value: String::from("tel:123"),
                    position: None
                })],
                position: None,
                url: String::from("tel:123"),
                title: None
            }),
            &Options {
                resource_link: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "[tel:123](tel:123)\n",
        "should use a resource link (`resourceLink: true`)"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            url: String::from("a"),
            title: None
        }),)
        .unwrap(),
        "[a](a)\n",
        "should use a normal link for nodes w/ a value similar to the url w/o a protocol"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: None
        }),)
        .unwrap(),
        "<tel:123>\n",
        "should use an autolink for nodes w/ a value similar to the url and a protocol"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: Some(String::from("a"))
        }),)
        .unwrap(),
        "[tel:123](tel:123 \"a\")\n",
        "should use a normal link for nodes w/ a value similar to the url w/ a title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a@b.c"),
                position: None
            })],
            position: None,
            url: String::from("mailto:a@b.c"),
            title: None
        }),)
        .unwrap(),
        "<a@b.c>\n",
        "should use an autolink for nodes w/ a value similar to the url and a protocol (email)"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a.b-c_d@a.b"),
                position: None
            })],
            position: None,
            url: String::from("mailto:a.b-c_d@a.b"),
            title: None
        }),)
        .unwrap(),
        "<a.b-c_d@a.b>\n",
        "should not escape in autolinks"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::new(),
                title: Some("b".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](<> 'b')\n",
        "should support a link w/ title when `quote: \"\'\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::new(),
                title: Some("'".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](<> '\\'')\n",
        "should escape a quote in `title` in a title when `quote: \"\'\"`'"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: "a b![c](d*e_f[g_h`i".to_string(),
            title: None
        }))
        .unwrap(),
        "[](<a b![c](d*e_f[g_h`i>)\n",
        "should not escape unneeded characters in a `DestinationLiteral`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: "a![b](c*d_e[f_g`h<i</j".to_string(),
            title: None
        }))
        .unwrap(),
        "[](a![b]\\(c*d_e[f_g`h<i</j)\n",
        "should not escape unneeded characters in a `DestinationRaw`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("#"),
            title: Some("a![b](c*d_e[f_g`h<i</j".to_string())
        }))
        .unwrap(),
        "[](# \"a![b](c*d_e[f_g`h<i</j\")\n",
        "should not escape unneeded characters in a `title` (double quotes)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::from("#"),
                title: Some("a![b](c*d_e[f_g`h<i</j".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](# 'a![b](c*d_e[f_g`h<i</j')\n",
        "should not escape unneeded characters in a `title` (single quotes)"
    );
}
