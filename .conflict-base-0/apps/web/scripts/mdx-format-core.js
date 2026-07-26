const YAML_DELIMITER = "---";

export const ARTICLE_FIELD_ORDER = [
  "meta_title",
  "display_title",
  "meta_description",
  "author",
  "created",
  "updated",
  "coverImage",
  "featured",
  "category",
];

export function createMdxFormatter(yaml) {
  function parse(content) {
    const lines = content.split("\n");

    if (lines[0] !== YAML_DELIMITER) {
      return { frontmatter: {}, body: content };
    }

    let endIndex = -1;
    for (let i = 1; i < lines.length; i++) {
      if (lines[i] === YAML_DELIMITER) {
        endIndex = i;
        break;
      }
    }

    if (endIndex === -1) {
      return { frontmatter: {}, body: content };
    }

    const yamlContent = lines.slice(1, endIndex).join("\n");
    const body = lines
      .slice(endIndex + 1)
      .join("\n")
      .replace(/^\n+/, "");

    try {
      const frontmatter = yaml.load(yamlContent) || {};
      return { frontmatter, body };
    } catch {
      return { frontmatter: {}, body: content };
    }
  }

  function stringify(frontmatter, body, fieldOrder) {
    const ordered = {};

    for (const key of fieldOrder) {
      if (frontmatter[key] !== undefined && frontmatter[key] !== "") {
        ordered[key] = frontmatter[key];
      }
    }

    for (const key of Object.keys(frontmatter)) {
      if (
        ordered[key] === undefined &&
        frontmatter[key] !== undefined &&
        frontmatter[key] !== ""
      ) {
        ordered[key] = frontmatter[key];
      }
    }

    const yamlStr = yaml.dump(ordered, {
      lineWidth: -1,
      quotingType: '"',
      forceQuotes: true,
      noRefs: true,
      sortKeys: false,
    });

    return `${YAML_DELIMITER}\n${yamlStr}${YAML_DELIMITER}\n\n${body}`;
  }

  function format(content, fieldOrder = ARTICLE_FIELD_ORDER) {
    const { frontmatter, body } = parse(content);

    if (Object.keys(frontmatter).length === 0) {
      return content;
    }

    return stringify(frontmatter, body, fieldOrder);
  }

  return { parse, stringify, format };
}
