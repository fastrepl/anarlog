// Builds a neutral return value for every generated plugin command from its tauri-specta
// binding, so unmocked commands return the shape the frontend expects (empty strings,
// arrays and objects) instead of null.
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const COMMAND =
  /async \w+\([^)]*\)\s*:\s*Promise<([\s\S]+?)>\s*\{\s*(?:try\s*\{\s*)?return\s*(?:\{\s*status:\s*"ok",\s*data:\s*)?await TAURI_INVOKE\(\s*"([^"]+)"/g;
const ALIAS_HEAD = /^export type (\w+)(?:<[^>]*>)?\s*=\s*/gm;

function stripComments(source) {
  let out = "";
  let quote = null;
  for (let index = 0; index < source.length; index++) {
    const char = source[index];
    if (quote) {
      out += char;
      if (char === "\\") {
        if (index + 1 < source.length) out += source[++index];
      } else if (char === quote) {
        quote = null;
      }
      continue;
    }
    if (char === '"' || char === "'" || char === "`") {
      quote = char;
      out += char;
      continue;
    }
    if (char === "/" && source[index + 1] === "/") {
      index += 2;
      while (index < source.length && source[index] !== "\n") index++;
      if (index < source.length) out += "\n";
      continue;
    }
    if (char === "/" && source[index + 1] === "*") {
      index += 2;
      let newlines = "";
      while (
        index < source.length &&
        !(source[index] === "*" && source[index + 1] === "/")
      ) {
        if (source[index] === "\n") newlines += "\n";
        index++;
      }
      index++;
      out += newlines || " ";
      continue;
    }
    out += char;
  }
  return out;
}

function parseAliases(source) {
  const aliases = new Map();
  for (const match of source.matchAll(ALIAS_HEAD)) {
    const start = match.index + match[0].length;
    let depth = 0;
    let inString = false;
    let end = source.length;
    for (let index = start; index < source.length; index++) {
      const char = source[index];
      if (inString) {
        if (char === "\\") index++;
        else if (char === '"') inString = false;
        continue;
      }
      if (char === '"') {
        inString = true;
      } else if ("<{([".includes(char)) {
        depth++;
      } else if (
        "})]".includes(char) ||
        (char === ">" && source[index - 1] !== "=")
      ) {
        depth = Math.max(0, depth - 1);
      } else if (depth === 0) {
        if (char === ";") {
          end = index;
          break;
        }
        if (char === "\n") {
          const body = source.slice(start, index).trim();
          const rest = source.slice(index + 1).trimStart();
          if (
            body &&
            !body.endsWith("|") &&
            !body.endsWith("&") &&
            !rest.startsWith("|") &&
            !rest.startsWith("&")
          ) {
            end = index;
            break;
          }
        }
      }
    }
    aliases.set(match[1], source.slice(start, end).replace(/\s+/g, " ").trim());
  }
  return aliases;
}

// Splits `a, b` or `a | b` at the top level, ignoring separators inside <>, {}, () and [].
function split(type, separator) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < type.length; index++) {
    const char = type[index];
    if ("<{([".includes(char)) depth++;
    else if (">})]".includes(char)) depth--;
    else if (depth === 0 && type.startsWith(separator, index)) {
      parts.push(type.slice(start, index).trim());
      start = index + separator.length;
    }
  }
  parts.push(type.slice(start).trim());
  return parts;
}

function fields(body) {
  const entries = [];
  for (const field of split(body, ";")) {
    const match = /^"?([\w$-]+)"?(\??)\s*:\s*(.+)$/s.exec(field.trim());
    if (match && !match[2]) entries.push([match[1], match[3].trim()]);
  }
  return entries;
}

function value(type, aliases, depth = 0) {
  type = type.trim();
  if (depth > 6) return null;
  const union = split(type, "|").filter(Boolean);
  if (union.length === 1 && union[0] !== type)
    return value(union[0], aliases, depth + 1);
  if (union.length > 1) {
    if (union.includes("null")) return null;
    const literal = union.find((part) => /^".*"$/.test(part));
    return literal ? JSON.parse(literal) : value(union[0], aliases, depth + 1);
  }
  if (type.startsWith("Result<"))
    return value(split(type.slice(7, -1), ",")[0], aliases, depth + 1);
  if (/^".*"$/.test(type)) return JSON.parse(type);
  if (type === "string") return "";
  if (type === "number") return 0;
  if (type === "boolean") return false;
  if (
    ["null", "void", "undefined", "unknown", "any", "JsonValue"].includes(type)
  )
    return null;
  if (type.endsWith("[]") || type.startsWith("Array<")) return [];
  if (type.startsWith("Partial<") || type.startsWith("{ [key")) return {};
  if (type.startsWith("{") && type.endsWith("}")) {
    return Object.fromEntries(
      fields(type.slice(1, -1)).map(([name, inner]) => [
        name,
        value(inner, aliases, depth + 1),
      ]),
    );
  }
  const alias = aliases.get(type.replace(/<.*$/, ""));
  return alias === undefined ? null : value(alias, aliases, depth + 1);
}

export function commandDefaults(repository) {
  const plugins = path.join(repository, "plugins");
  const bindings = [
    path.join(repository, "apps/desktop/src/types/tauri.gen.ts"),
    ...readdirSync(plugins).map((plugin) =>
      path.join(plugins, plugin, "js/bindings.gen.ts"),
    ),
  ];
  const defaults = {};
  for (const file of bindings) {
    let source;
    try {
      source = readFileSync(file, "utf8");
    } catch {
      continue;
    }
    const stripped = stripComments(source);
    const aliases = parseAliases(stripped);
    for (const [, type, command] of stripped.matchAll(COMMAND)) {
      defaults[command] = value(type.replace(/\s+/g, " "), aliases);
    }
  }
  // Auto's template page renders the bundled summary format.
  defaults["plugin:template|get_template_source"] = readFileSync(
    path.join(repository, "crates/template-app/assets/enhance.format.md.jinja"),
    "utf8",
  );
  return defaults;
}
