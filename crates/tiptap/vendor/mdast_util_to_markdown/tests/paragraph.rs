use markdown::mdast::{Node, Paragraph, Text};
use mdast_util_to_markdown::to_markdown as to;
use pretty_assertions::assert_eq;

#[test]
fn paragraph() {
    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![],
            position: None
        }))
        .unwrap(),
        "",
        "should support an empty paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\nb"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\nb\n",
        "should support a paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("  a"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "&#x20; a\n",
        "should encode spaces at the start of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a  "),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a &#x20;\n",
        "should encode spaces at the end of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("\t\ta"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "&#x9;\ta\n",
        "should encode tabs at the start of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\t\t"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\t&#x9;\n",
        "should encode tabs at the end of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a  \n  b"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a &#x20;\n&#x20; b\n",
        "should encode spaces around line endings in paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\t\t\n\t\tb"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\t&#x9;\n&#x9;\tb\n",
        "should encode spaces around line endings in paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("я_я"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "я\\_я\n",
        "should support escaping around non-ascii"
    );
}
