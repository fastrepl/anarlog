use markdown::mdast::{List, ListItem, Node, Paragraph, Text, ThematicBreak};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, IndentOptions, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn list() {
    assert_eq!(
        to(&Node::List(List {
            children: vec![],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "",
        "should support an empty list"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![Node::ListItem(ListItem {
                children: Vec::new(),
                position: None,
                spread: false,
                checked: None
            })],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "*\n",
        "should support a list w/ an item"
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
                            value: String::from("b"),
                            position: None
                        })],
                        position: None
                    })],
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
        "- a\n- ***\n- b\n",
        "should support a list w/ items"
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
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "- a\n- ***\n",
        "should not use blank lines between items for lists w/ `spread: false`"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
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
                        })
                    ],
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
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "- a\n\n  b\n- ***\n",
        "should support a list w/ `spread: false`, w/ a spread item"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![Node::ListItem(ListItem {
                children: Vec::new(),
                position: None,
                spread: false,
                checked: None
            })],
            position: None,
            ordered: true,
            start: None,
            spread: false
        }))
        .unwrap(),
        "1.\n",
        "should support a list w/ `ordered` and an empty item"
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
                            value: String::from("b"),
                            position: None
                        }),],
                        position: None
                    })],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: true,
            start: None,
            spread: false
        }))
        .unwrap(),
        "1. a\n2. ***\n3. b\n",
        "should support a list w/ `ordered`"
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
                            value: String::from("b"),
                            position: None
                        })],
                        position: None
                    })],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: true,
            start: None,
            spread: false
        }))
        .unwrap(),
        "1. a\n2. ***\n3. b\n",
        "should support a list w/ `ordered` and `spread: false`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
                children: vec![
                    Node::ListItem(ListItem {
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
                                value: String::from("b"),
                                position: None
                            })],
                            position: None
                        })],
                        position: None,
                        spread: false,
                        checked: None
                    })
                ],
                position: None,
                ordered: true,
                start: None,
                spread: false
            }),
            &Options {
                increment_list_marker: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "1. a\n1. ***\n1. b\n",
        "should support a list w/ `ordered` when `increment_list_marker: false`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
                children: vec![
                    Node::ListItem(ListItem {
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
                    }),
                    Node::ListItem(ListItem {
                        children: vec![Node::ThematicBreak(ThematicBreak { position: None })],
                        position: None,
                        spread: false,
                        checked: None
                    })
                ],
                position: None,
                ordered: true,
                start: Some(0),
                spread: false
            }),
            &Options {
                list_item_indent: IndentOptions::One,
                ..Default::default()
            }
        )
        .unwrap(),
        "0. a\n1. ***\n",
        "should support a list w/ `ordered` and `start`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
                children: vec![
                    Node::ListItem(ListItem {
                        children: vec![Node::Paragraph(Paragraph {
                            children: vec![Node::Text(Text {
                                value: String::from("a\nb"),
                                position: None
                            })],
                            position: None
                        })],
                        position: None,
                        spread: false,
                        checked: None
                    }),
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
                ],
                position: None,
                ordered: false,
                start: None,
                spread: false
            }),
            &Options {
                list_item_indent: IndentOptions::Mixed,
                ..Default::default()
            }
        )
        .unwrap(),
        "* a\n  b\n* c\n  d\n",
        "should support a correct prefix and indent `list_item_indent: IndentOptions::Mixed` and a tight list"
    );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: false,
                   start: None,
                   spread:true
               }),
               &Options {
                   list_item_indent: IndentOptions::Mixed,
                   ..Default::default()
               }
           )
           .unwrap(),
           "*   a\n    b\n\n*   c\n    d\n",
           "should support a correct prefix and indent `list_item_indent: IndentOptions::Mixed` and a tight list"
       );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: true,
                   start: Some(9),
                   spread: false
               }),
               &Options {
                   list_item_indent: IndentOptions::One,
                   ..Default::default()
               }
           )
           .unwrap(),
           "9. a\n   b\n10. c\n    d\n",
           "should support a correct prefix and indent for items 9 and 10 when `list_item_indent: IndentOptions::One`"
       );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: true,
                   start: Some(99),
                   spread: false
               }),
               &Options {
                   list_item_indent: IndentOptions::One,
                   ..Default::default()
               }
           )
           .unwrap(),
           "99. a\n    b\n100. c\n     d\n",
           "should support a correct prefix and indent for items 90 and 100 when `list_item_indent: IndentOptions::One`"
       );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: true,
                   start: Some(999),
                   spread: false
               }),
               &Options {
                   list_item_indent: IndentOptions::One,
                   ..Default::default()
               }
           )
           .unwrap(),
           "999. a\n     b\n1000. c\n      d\n",
           "should support a correct prefix and indent for items 999 and 1000 when `list_item_indent: IndentOptions::One`"
       );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: true,
                   start: Some(9),
                   spread: false
               }),
               &Options {
                   list_item_indent: IndentOptions::Tab,
                   ..Default::default()
               }
           )
           .unwrap(),
           "9.  a\n    b\n10. c\n    d\n",
           "should support a correct prefix and indent for items 9 and 10  when `list_item_indent: IndentOptions::Tab`"
       );

    assert_eq!(
           to_md_with_opts(
               &Node::List(List {
                   children: vec![
                       Node::ListItem(ListItem {
                           children: vec![Node::Paragraph(Paragraph {
                               children: vec![Node::Text(Text {
                                   value: String::from("a\nb"),
                                   position: None
                               }),],
                               position: None
                           })],
                           position: None,
                           spread: false,
                           checked: None
                       }),
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
                   ],
                   position: None,
                   ordered: true,
                   start: Some(99),
                   spread: false
               }),
               &Options {
                   list_item_indent: IndentOptions::Tab,
                   ..Default::default()
               }
           )
           .unwrap(),
           "99. a\n    b\n100.    c\n        d\n",
           "should support a correct prefix and indent for items 99 and 100 when `list_item_indent: IndentOptions::Tab`"
       );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
                children: vec![
                    Node::ListItem(ListItem {
                        children: vec![Node::Paragraph(Paragraph {
                            children: vec![Node::Text(Text {
                                value: String::from("a\nb"),
                                position: None
                            }),],
                            position: None
                        })],
                        position: None,
                        spread: false,
                        checked: None
                    }),
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
                ],
                position: None,
                ordered: true,
                start: Some(999),
                spread: false
            }),
            &Options {
                list_item_indent: IndentOptions::Tab,
                ..Default::default()
            }
        )
        .unwrap(),
        "999.    a\n        b\n1000.   c\n        d\n",
        "should support a correct prefix and indent for items 999 and 1000 when `list_item_indent: IndentOptions::Tab`"
    );
}
