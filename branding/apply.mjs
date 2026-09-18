#!/usr/bin/env node
// Re-derives this fork's brand from branding/brand.json across the tracked tree.
//
// The fork renames upstream's product name in ~1500 places. Hand-editing those
// lines turns every upstream commit that touches the same copy into a merge
// conflict. Instead the rename is a pure function of the tree: after a merge
// takes upstream's side of a branded line, running this puts the brand back.
// It is idempotent, so running it on an already-branded tree is a no-op.

import { execFileSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const brand = JSON.parse(
  readFileSync(resolve(root, "branding/brand.json"), "utf8"),
);
const check = process.argv.includes("--check");

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// An identifier character, path separator, dot or hyphen on either side means
// the occurrence is part of a symbol, package name, URL scheme or bundle id --
// never user-facing copy.
const EDGE = "[A-Za-z0-9_/\\-.]";

// Ordered alternation: a protected token is matched (and returned untouched)
// before the bare brand name can match inside it.
const pattern = new RegExp(
  `(${brand.protected.map(escape).join("|")})|(?<!${EDGE})${escape(brand.upstream)}(?!${EDGE})`,
  "g",
);

// A licence or notice names the work it was granted for. Retitling upstream's
// commercial licence after this fork would misstate what the agreement covers,
// so these are off limits wherever they sit in the tree.
const isLegal = (f) => /^(LICEN[CS]E|COPYING|NOTICE)/i.test(f.split("/").pop());

// Fix the article before renaming, so "an Anarlog account" does not become
// "an BlackMushi account". Capitalisation of the article is preserved.
const article = brand.article;
const articlePattern = article
  ? new RegExp(
      `\\b(${article.upstream})(\\s+)(?=${escape(brand.upstream)}\\b)`,
      "gi",
    )
  : null;

const matchCase = (sample, word) =>
  sample[0] === sample[0].toUpperCase()
    ? word[0].toUpperCase() + word.slice(1)
    : word;

const substitute = (text) => {
  const withArticle = articlePattern
    ? text.replace(
        articlePattern,
        (m, art, gap) => matchCase(art, article.fork) + gap,
      )
    : text;
  return withArticle.replace(
    pattern,
    (m, protectedToken) => protectedToken ?? brand.fork,
  );
};

// In Rust a bare `Anarlog` is an identifier -- an enum variant, a type, a module.
// `AdapterKind::Anarlog` names upstream's hosted STT provider alongside Together
// and Speechmatics; renaming it would be plain wrong, and the edge-character
// guard cannot tell it from copy because `::` and `,` are not word characters.
// User-facing Rust text lives in string literals and in the doc comments clap
// turns into CLI help, so only those are rewritten.
const RUST_TEXT =
  /r#*"[\s\S]*?"#*|"(?:[^"\\\n]|\\.)*"|\/\/[^\n]*|\/\*[\s\S]*?\*\//g;
const rewrite = (text, file) =>
  file.endsWith(".rs") ? text.replace(RUST_TEXT, substitute) : substitute(text);

const tracked = execFileSync("git", ["ls-files"], {
  cwd: root,
  maxBuffer: 1 << 28,
})
  .toString("utf8")
  .split("\n")
  .filter(Boolean)
  .filter((f) => !isLegal(f))
  .filter((f) => !brand.exclude.some((e) => f === e || f.startsWith(e)));

const changed = [];
for (const file of tracked) {
  const abs = resolve(root, file);
  let stat;
  try {
    stat = statSync(abs);
  } catch {
    continue; // staged for deletion, or absent from the working tree
  }
  if (!stat.isFile() || stat.size > 4 << 20) continue;

  const buf = readFileSync(abs);
  if (buf.includes(0)) continue; // binary
  const before = buf.toString("utf8");
  if (!before.includes(brand.upstream)) continue;

  const after = rewrite(before, file);
  if (after !== before) {
    changed.push(file);
    if (!check) writeFileSync(abs, after);
  }
}

if (check) {
  if (changed.length) {
    console.error(`brand overlay is stale in ${changed.length} file(s):`);
    for (const f of changed.slice(0, 20)) console.error(`  ${f}`);
    if (changed.length > 20)
      console.error(`  ... and ${changed.length - 20} more`);
    console.error("\nrun: node branding/apply.mjs");
    process.exit(1);
  }
  console.log("brand overlay is up to date");
} else {
  console.log(
    changed.length
      ? `rebranded ${changed.length} file(s): ${brand.upstream} -> ${brand.fork}`
      : `nothing to do (tree already reads ${brand.fork})`,
  );
}
