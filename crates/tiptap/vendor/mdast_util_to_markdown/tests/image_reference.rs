use markdown::mdast::{ImageReference, Node, Paragraph, ReferenceKind};
use mdast_util_to_markdown::to_markdown as to;
use pretty_assertions::assert_eq;

#[test]
fn image_reference() {
    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::new(),
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "![][]\n",
        "should support a link reference (nonsensical)"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::from("a"),
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "![a][]\n",
        "should support `alt`"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::new(),
            reference_kind: ReferenceKind::Full,
            identifier: String::from("a"),
            label: None
        }))
        .unwrap(),
        "![][a]\n",
        "should support an `identifier` (nonsensical)"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::new(),
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: String::from("a").into()
        }))
        .unwrap(),
        "![][a]\n",
        "should support a `label` (nonsensical)"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::from("A"),
            reference_kind: ReferenceKind::Shortcut,
            identifier: String::from("A"),
            label: None
        }))
        .unwrap(),
        "![A]\n",
        "should support `reference_kind: \"ReferenceKind::Shortcut\"`"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::from("A"),
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("A"),
            label: None
        }))
        .unwrap(),
        "![A][]\n",
        "should support `reference_kind: \"ReferenceKind::Collapsed\"`"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::from("A"),
            reference_kind: ReferenceKind::Full,
            identifier: String::from("A"),
            label: None
        }))
        .unwrap(),
        "![A][A]\n",
        "should support `reference_kind: \"ReferenceKind::Full\"`"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            alt: String::from("&"),
            label: String::from("&").into(),
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
        }))
        .unwrap(),
        "![&][&]\n",
        "should prefer label over identifier"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            label: None,
            alt: String::from("&"),
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
        }))
        .unwrap(),
        "![&][&]\n",
        "should decode `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::ImageReference(ImageReference {
                position: None,
                label: None,
                alt: String::from("&a;"),
                reference_kind: ReferenceKind::Full,
                identifier: String::from("&b;"),
            })],
            position: None
        }))
        .unwrap(),
        "![\\&a;][&b;]\n",
        "should support incorrect character references"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            label: None,
            alt: String::from("+"),
            reference_kind: ReferenceKind::Full,
            identifier: String::from("\\+"),
        }))
        .unwrap(),
        "![+][+]\n",
        "should unescape `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::ImageReference(ImageReference {
            position: None,
            label: None,
            alt: String::from("a"),
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("b"),
        }))
        .unwrap(),
        "![a][b]\n",
        "should use a full reference if w/o `ReferenceKind` and the label does not match the reference"
    );
}
