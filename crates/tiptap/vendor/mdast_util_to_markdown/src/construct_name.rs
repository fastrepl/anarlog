//! Names of the things being serialized.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/index.d.ts#L18.

#[derive(Clone, PartialEq)]
pub enum ConstructName {
    /// Whole autolink.
    ///
    /// ```markdown
    /// > | <https://example.com> and <admin@example.com>
    ///     ^^^^^^^^^^^^^^^^^^^^^     ^^^^^^^^^^^^^^^^^^^
    /// ```
    Autolink,
    /// Whole block quote.
    ///
    /// ```markdown
    /// > | > a
    ///     ^^^
    /// > | b
    ///     ^
    /// ```
    Blockquote,
    /// Whole code (fenced).
    ///
    /// ````markdown
    /// > | ```js
    ///     ^^^^^
    /// > | console.log(1)
    ///     ^^^^^^^^^^^^^^
    /// > | ```
    ///     ^^^
    /// ````
    CodeFenced,
    /// Code (fenced) language, when fenced with grave accents.
    ///
    /// ````markdown
    /// > | ```js
    ///        ^^
    ///   | console.log(1)
    ///   | ```
    /// ````
    CodeFencedLangGraveAccent,
    /// Code (fenced) language, when fenced with tildes.
    ///
    /// ````markdown
    /// > | ~~~js
    ///        ^^
    ///   | console.log(1)
    ///   | ~~~
    /// ````
    CodeFencedLangTilde,
    /// Code (fenced) meta string, when fenced with grave accents.
    ///
    /// ````markdown
    /// > | ```js eval
    ///           ^^^^
    ///   | console.log(1)
    ///   | ```
    /// ````
    CodeFencedMetaGraveAccent,
    /// Code (fenced) meta string, when fenced with tildes.
    ///
    /// ````markdown
    /// > | ~~~js eval
    ///           ^^^^
    ///   | console.log(1)
    ///   | ~~~
    /// ````
    CodeFencedMetaTilde,
    /// Whole code (indented).
    ///
    /// ```markdown
    /// ␠␠␠␠console.log(1)
    /// ^^^^^^^^^^^^^^^^^^
    /// ```
    CodeIndented,
    /// Whole definition.
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///     ^^^^^^^^^^
    /// ```
    Definition,
    /// Destination (literal) (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a]: <b> "c"
    ///          ^^^
    /// > | a ![b](<c> "d") e
    ///            ^^^
    /// ```
    DestinationLiteral,
    /// Destination (raw) (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///          ^
    /// > | a ![b](c "d") e
    ///            ^
    /// ```
    DestinationRaw,
    /// Emphasis.
    ///
    /// ```markdown
    /// > | *a*
    ///     ^^^
    /// ```
    Emphasis,
    /// Whole heading (atx).
    ///
    /// ```markdown
    /// > | # alpha
    ///     ^^^^^^^
    /// ```
    HeadingAtx,
    /// Whole heading (setext).
    ///
    /// ```markdown
    /// > | alpha
    ///     ^^^^^
    /// > | =====
    ///     ^^^^^
    /// ```
    HeadingSetext,
    /// Whole image.
    ///
    /// ```markdown
    /// > | ![a](b)
    ///     ^^^^^^^
    /// > | ![c]
    ///     ^^^^
    /// ```
    Image,
    /// Whole image reference.
    ///
    /// ```markdown
    /// > | ![a]
    ///     ^^^^
    /// ```
    ImageReference,
    /// Label (occurs in definitions, image reference, image, link reference,
    /// link).
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///     ^^^
    /// > | a [b] c
    ///       ^^^
    /// > | a ![b][c] d
    ///       ^^^^
    /// > | a [b](c) d
    ///       ^^^
    /// ```
    Label,
    /// Whole link.
    ///
    /// ```markdown
    /// > | [a](b)
    ///     ^^^^^^
    /// > | [c]
    ///     ^^^
    /// ```
    Link,
    /// Whole link reference.
    ///
    /// ```markdown
    /// > | [a]
    ///     ^^^
    /// ```
    LinkReference,
    /// List.
    ///
    /// ```markdown
    /// > | * a
    ///     ^^^
    /// > | 1. b
    ///     ^^^^
    /// ```
    List,
    /// List item.
    ///
    /// ```markdown
    /// > | * a
    ///     ^^^
    /// > | 1. b
    ///     ^^^^
    /// ```
    ListItem,
    /// Math (flow).
    ///
    /// ```markdown
    /// > | $$
    ///     ^^
    /// > | a
    ///     ^
    /// > | $$
    ///     ^^
    /// ```
    MathFlow,
    /// Math (flow) meta flag.
    ///
    /// ```markdown
    /// > | $$a
    ///       ^
    ///   | b
    ///   | $$
    /// ```
    MathFlowMeta,
    /// Paragraph.
    ///
    /// ```markdown
    /// > | a b
    ///     ^^^
    /// > | c.
    ///     ^^
    /// ```
    Paragraph,
    /// Phrasing (occurs in headings, paragraphs, etc).
    ///
    /// ```markdown
    /// > | a
    ///     ^
    /// ```
    Phrasing,
    /// Reference (occurs in image, link).
    ///
    /// ```markdown
    /// > | [a][]
    ///        ^^
    /// ```
    Reference,
    /// Strong.
    ///
    /// ```markdown
    /// > | **a**
    ///     ^^^^^
    /// ```
    Strong,
    /// Title using single quotes (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a](b 'c')
    ///           ^^^
    /// ```
    TitleApostrophe,
    /// Title using double quotes (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a](b "c")
    ///           ^^^
    /// ```
    TitleQuote,
}
