use markdown::mdast::{LinkReference, Node, Paragraph, ReferenceKind, Text};
use mdast_util_to_markdown::to_markdown as to;
use pretty_assertions::assert_eq;

#[test]
fn link_reference() {
    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "[][]\n",
        "should support a link reference (nonsensical"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "[a][]\n",
        "should support `children`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("a"),
            label: None
        }))
        .unwrap(),
        "[][a]\n",
        "should support an `identifier` (nonsensical)"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: Some(String::from("a")),
        }))
        .unwrap(),
        "[][a]\n",
        "should support a `label` (nonsensical)"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Shortcut,
            identifier: String::from("A"),
            label: None
        }))
        .unwrap(),
        "[A]\n",
        "should support `reference_type: ReferenceKind::Shortcut`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("A"),
            label: Some("A".into())
        }))
        .unwrap(),
        "[A][]\n",
        "should support `reference_type: ReferenceKind::Collapsed`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("A"),
            label: Some("A".into())
        }))
        .unwrap(),
        "[A][A]\n",
        "should support `reference_type: ReferenceKind::Full`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("&"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
            label: Some("&".into())
        }))
        .unwrap(),
        "[&][&]\n",
        "should prefer label over identifier"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("&"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
            label: None
        }))
        .unwrap(),
        "[&][&]\n",
        "should decode `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::LinkReference(LinkReference {
                position: None,
                label: None,
                children: vec![Node::Text(Text {
                    value: String::from("&a;"),
                    position: None
                })],
                reference_kind: ReferenceKind::Full,
                identifier: String::from("&b;"),
            })],
            position: None
        }))
        .unwrap(),
        "[\\&a;][&b;]\n",
        "should support incorrect character references"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("a![b](c*d_e[f_g`h<i</j"),
            label: None
        }))
        .unwrap(),
        "[][a!\\[b\\](c*d_e\\[f_g`h<i</j]\n",
        "should not escape unneeded characters in a `reference`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("+"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("\\+"),
            label: None
        }))
        .unwrap(),
        "[+][+]\n",
        "should unescape `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("a"),
            label: Some("b".to_string())
        }))
        .unwrap(),
        "[a][b]\n",
        "should use `reference_type: ReferenceKind::Full` if the label doesn't match the reference"
    );
}
