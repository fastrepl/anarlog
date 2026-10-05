use markdown::mdast::{Image, Node};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn image() {
    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "![]()\n",
        "should support an image"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::from("a"),
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "![a]()\n",
        "should support `alt`"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("a"),
            title: None
        }))
        .unwrap(),
        "![](a)\n",
        "should support a url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::new(),
            title: Some(String::from("a"))
        }))
        .unwrap(),
        "![](<> \"a\")\n",
        "should support a title"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("a"),
            title: Some(String::from("b"))
        }))
        .unwrap(),
        "![](a \"b\")\n",
        "should support a url and title"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b c"),
            title: None
        }))
        .unwrap(),
        "![](<b c>)\n",
        "should support an image w/ enclosed url w/ whitespace in url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b <c"),
            title: None
        }))
        .unwrap(),
        "![](<b \\<c>)\n",
        "should escape an opening angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b >c"),
            title: None
        }))
        .unwrap(),
        "![](<b \\>c>)\n",
        "should escape a closing angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b \\+c"),
            title: None
        }))
        .unwrap(),
        "![](<b \\\\+c>)\n",
        "should escape a backslash in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b\nc"),
            title: None
        }))
        .unwrap(),
        "![](<b&#xA;c>)\n",
        "should encode a line ending in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b(c"),
            title: None
        }))
        .unwrap(),
        "![](b\\(c)\n",
        "should escape an opening paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b)c"),
            title: None
        }))
        .unwrap(),
        "![](b\\)c)\n",
        "should escape a closing paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("b\\+c"),
            title: None
        }))
        .unwrap(),
        "![](b\\\\+c)\n",
        "should escape a backslash in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::from("\x0C"),
            title: None
        }))
        .unwrap(),
        "![](<\x0C>)\n",
        "should support control characters in images"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::new(),
            title: Some(String::from("b\"c"))
        }))
        .unwrap(),
        "![](<> \"b\\\"c\")\n",
        "should escape a double quote in `title`"
    );

    assert_eq!(
        to(&Node::Image(Image {
            position: None,
            alt: String::new(),
            url: String::new(),
            title: Some(String::from("b\\.c"))
        }))
        .unwrap(),
        "![](<> \"b\\\\.c\")\n",
        "should escape a backslash in `title`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Image(Image {
                position: None,
                alt: String::new(),
                url: String::new(),
                title: Some(String::from("b"))
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "![](<> 'b')\n",
        "should support an image w/ title when `quote: \"\'\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Image(Image {
                position: None,
                alt: String::new(),
                url: String::new(),
                title: Some(String::from("'"))
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "![](<> '\\'')\n",
        "should escape a quote in `title` in a title when `quote: \"\'\"`"
    );
}
