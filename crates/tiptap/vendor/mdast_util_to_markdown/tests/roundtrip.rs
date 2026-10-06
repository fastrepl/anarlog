use markdown::{mdast::Node, to_mdast as from};
use mdast_util_to_markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
use pretty_assertions::assert_eq;

#[test]
fn roundtrip() {
    let doc: String = document(vec![
        "> * Lorem ipsum dolor sit amet",
        ">",
        "> * consectetur adipisicing elit",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "* Lorem ipsum dolor sit amet",
        "",
        "  1. consectetur adipisicing elit",
        "",
        "  2. sed do eiusmod tempor incididunt",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "* 1. Lorem ipsum dolor sit amet",
        "",
        "  2. consectetur adipisicing elit",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "* hello",
        "  * world",
        "    how",
        "",
        "    are",
        "    you",
        "",
        "  * today",
        "* hi",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "An autolink: <http://example.com/?foo=1&bar=2>.\n".to_string();

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "A [primary][toString], [secondary][constructor], and [tertiary][__proto__] link.",
        "",
        "[toString]: http://primary.com",
        "",
        "[__proto__]: http://tertiary.com",
        "",
        "[constructor]: http://secondary.com",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "* foo",
        "",
        "*",
        "",
        "* bar",
        "",
        "* baz",
        "",
        "*",
        "",
        "* qux quux",
        "",
    ]);

    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "* a\n\n<!---->\n\n* b\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = document(vec![
        "    <h3>Header 3</h3>",
        "",
        "    <blockquote>",
        "        <p>This is a blockquote.</p>",
        "        ",
        "        <p>This is the second paragraph in the blockquote.</p>",
        "        ",
        "        <h2>This is an H2 in a blockquote</h2>",
        "    </blockquote>",
        "",
    ]);

    assert_eq!(
        to_md_with_opts(
            &from(&doc, &Default::default()).unwrap(),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        doc
    );

    let doc: String = "> a\n\n> b\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "[**https://unifiedjs.com/**](https://unifiedjs.com/)\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let step1 = "\\ \\\\ \\\\\\ \\\\\\\\";
    let step2 = "\\ \\ \\\\\\ \\\\\\\\\n";
    assert_eq!(
        to(&from(step1, &Default::default()).unwrap()).unwrap(),
        step2
    );
    assert_eq!(
        to(&from(step2, &Default::default()).unwrap()).unwrap(),
        step2
    );

    let doc = "\\\\\\*a\n";
    assert_eq!(to(&from(doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc = "\\\\*a\\\\\\*";
    assert_eq!(
        remove_pos(&mut from(doc, &Default::default()).unwrap()),
        remove_pos(
            &mut from(
                &to(&from(doc, &Default::default()).unwrap()).unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let doc = "```\n	\n```\n";
    assert_eq!(to(&from(doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc = "* * -\n";
    assert_eq!(to(&from(doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc = "- ***\n";
    assert_eq!(to(&from(doc, &Default::default()).unwrap()).unwrap(), doc);

    let mut tree = from("* a\n- b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("* ---\n- - +\n+ b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("- - +\n* ---\n+ b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("- - +\n- -", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("* - +\n    *\n    -\n    +", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("- +\n- *\n  -\n  +", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(
            &mut from(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let mut tree = from("1. a\n1) b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. ---\n1) 1. 1)\n1. b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. 1. 1)\n1) ---\n1. b", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. 1. 1)\n1. 1.", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. 1) 1.\n      1.\n      1)\n    1.", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. 1) 1.\n   1) 1.\n     1)\n     1.", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let mut tree = from("1. 1)\n1. 1.\n   1)\n   1.", &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut tree),
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let doc: String = "&#x20;\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "&#x9;\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "&#x20; a &#x20;\n&#x9;\tb\t&#x9;\n".to_string();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = "Separate paragraphs:

a * is this emphasis? *

a ** is this emphasis? **

a *** is this emphasis? ***

a *\\* is this emphasis? *\\*

a \\** is this emphasis? \\**

a **\\* is this emphasis? **\\*

a *\\** is this emphasis? *\\**

One paragraph:

a * is this emphasis? *
a ** is this emphasis? **
a *** is this emphasis? ***
a *\\* is this emphasis? *\\*
a \\** is this emphasis? \\**
a **\\* is this emphasis? **\\*
a *\\** is this emphasis? *\\**"
        .to_string();
    let mut tree = from(&doc, &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap()),
        remove_pos(&mut tree),
    );

    let doc: String = "Separate paragraphs:

a _ is this emphasis? _

a __ is this emphasis? __

a ___ is this emphasis? ___

a _\\_ is this emphasis? _\\_

a \\__ is this emphasis? \\__

a __\\_ is this emphasis? __\\_

a _\\__ is this emphasis? _\\__

One paragraph:

a _ is this emphasis? _
a __ is this emphasis? __
a ___ is this emphasis? ___
a _\\_ is this emphasis? _\\_
a \\__ is this emphasis? \\__
a __\\_ is this emphasis? __\\_
a _\\__ is this emphasis? _\\__"
        .to_string();
    let mut tree = from(&doc, &Default::default()).unwrap();
    assert_eq!(
        remove_pos(&mut from(&to(&tree).unwrap(), &Default::default()).unwrap()),
        remove_pos(&mut tree),
    );

    let doc: String = to(&from("(____", &Default::default()).unwrap()).unwrap();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);

    let doc: String = to(&from(
        "Once activated, a service worker ______, then transitions to idle…",
        &Default::default(),
    )
    .unwrap())
    .unwrap();
    assert_eq!(to(&from(&doc, &Default::default()).unwrap()).unwrap(), doc);
}

fn remove_pos(node: &mut Node) {
    node.position_set(None);
    if let Some(children) = node.children_mut() {
        for child in children {
            remove_pos(child);
        }
    }
}

fn document(doc: Vec<&str>) -> String {
    doc.join("\n")
}
