use markdown::mdast::{List, ListItem, Node, Paragraph, Root, Text, ThematicBreak};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, IndentOptions, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn list_item() {
    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "*\n",
        "should support a list item"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: Vec::new(),
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                bullet: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "+\n",
        "should serialize an item w/ a plus as bullet when `bullet: \" + \"`"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None
            })],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n",
        "should support a list item w/ a child"
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
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n  ***\n  b\n",
        "should support a list item w/ children"
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
                    Node::ThematicBreak(ThematicBreak { position: None })
                ],
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::One,
                ..Default::default()
            }
        )
        .unwrap(),
        "* a\n  ***\n",
        "should use one space after the bullet for `list_item_indent: \"IndentOptions::One\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),],
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::Mixed,
                ..Default::default()
            }
        )
        .unwrap(),
        "* a\n",
        "should use one space after the bullet for `list_item_indent: \"IndentOptions::Mixed\"`, when the item is not spread"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::ThematicBreak(ThematicBreak { position: None })],
                position: None,
                spread: true,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::Mixed,
                ..Default::default()
            }
        )
        .unwrap(),
        "*   a\n\n    ***\n",
        "should use a tab stop of spaces after the bullet for `list_item_indent: \"IndentOptions::Mixed\"`, when the item is spread"
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
                Node::ThematicBreak(ThematicBreak { position: None }),
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "* a\n  ***\n",
        "should not use blank lines between child blocks for items w/ `spread: false`"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet_other: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "* * +\n",
        "should support `bullet_other`"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '-',
                ..Default::default()
            }
        )
        .unwrap(),
        "- - *\n",
        "should default to an `bullet_other` different from `bullet` (1)"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '*',
                ..Default::default()
            }
        )
        .unwrap(),
        "* * -\n",
        "should default to an `bullet_other` different from `bullet` (2)"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
                    children: vec![Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("a"),
                            position: None
                        })],
                        position: None
                    }),],
                    position: None,
                    spread: false,
                    checked: None
                }),
                Node::ListItem(ListItem {
                    children: vec![Node::ThematicBreak(ThematicBreak { position: None })],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "- a\n- ***\n",
        "should use a different bullet than a thematic rule marker, if the first child of a list item is a thematic break (2)"
    );

    assert_eq!(
        to(&create_list(create_list::<Option<Node>>(None))).unwrap(),
        "* *\n",
        "should *not* use a different bullet for an empty list item in two lists"
    );

    assert_eq!(
        to(&create_list(create_list(create_list::<Option<Node>>(None)))).unwrap(),
        "* * -\n",
        "should use a different bullet for an empty list item in three lists (1)"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
                    children: vec![],
                    position: None,
                    spread: false,
                    checked: None
                }),
                Node::ListItem(ListItem {
                    children: vec![create_list(create_list::<Option<Node>>(None))],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "*\n* * -\n",
        "should use a different bullet for an empty list item in three lists (2)"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "+ + +\n",
        "should not use a different bullet for an empty list item in three lists if `bullet` isn’t a thematic rule marker"
    );

    assert_eq!(
        to(&create_list(create_list(create_list(create_list::<
            Option<Node>,
        >(None)))))
        .unwrap(),
        "* * * -\n",
        "should use a different bullet for an empty list item in four lists"
    );

    assert_eq!(
        to(&create_list(create_list(create_list(create_list(
            create_list::<Option<Node>>(None)
        )))))
        .unwrap(),
        "* * * * -\n",
        "should use a different bullet for an empty list item in five lists"
    );

    assert_eq!(
        to(&create_list(create_list(vec![
            create_list(Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None
            })),
            create_list::<Option<Node>>(None)
        ])))
        .unwrap(),
        "* * * a\n    -\n",
        "should not use a different bullet for an empty list item at non-head in two lists"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
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
            &Options {
                bullet_ordered: ')',
                ..Default::default()
            }
        )
        .unwrap(),
        "1)\n",
        "should support `bullet_ordered`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Root(Root {
                children: vec![
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
                ],
                position: None
            }),
            &Options {
                bullet_ordered: ')',
                ..Default::default()
            }
        )
        .unwrap(),
        "1)\n\n1.\n",
        "should use a different bullet for adjacent ordered lists"
    );
}

trait IntoVecNode {
    fn into_vec(self) -> Vec<Node>;
}

impl IntoVecNode for Node {
    fn into_vec(self) -> Vec<Node> {
        vec![self]
    }
}

impl IntoVecNode for Option<Node> {
    fn into_vec(self) -> Vec<Node> {
        self.map(|n| vec![n]).unwrap_or_default()
    }
}

impl IntoVecNode for Vec<Node> {
    fn into_vec(self) -> Vec<Node> {
        self
    }
}

fn create_list<T>(d: T) -> Node
where
    T: IntoVecNode,
{
    Node::List(List {
        children: vec![Node::ListItem(ListItem {
            children: d.into_vec(),
            position: None,
            spread: false,
            checked: None,
        })],
        position: None,
        ordered: false,
        start: None,
        spread: false,
    })
}
