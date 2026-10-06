use markdown::mdast::{Html, Node, Paragraph, Text};
use mdast_util_to_markdown::to_markdown as to;
use pretty_assertions::assert_eq;

#[test]
fn html() {
    assert_eq!(
        to(&Node::Html(Html {
            value: String::new(),
            position: None
        }))
        .unwrap(),
        "",
        "should support an empty html"
    );

    assert_eq!(
        to(&Node::Html(Html {
            value: String::from("a\nb"),
            position: None
        }))
        .unwrap(),
        "a\nb\n",
        "should support html"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (1)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (2)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: "a\r\n".to_string(),
                    position: None
                }),
                Node::Html(Html {
                    value: "<div>".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "a <div>\n",
        "should prevent html (text) from becoming html (flow) (3)"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Html(Html {
                    value: "<x>".to_string(),
                    position: None
                }),
                Node::Text(Text {
                    value: "a".to_string(),
                    position: None
                })
            ],
            position: None
        }))
        .unwrap(),
        "<x>a\n",
        "should serialize html (text)"
    );
}
