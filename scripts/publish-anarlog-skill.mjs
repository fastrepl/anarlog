import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

// skills/anarlog/SKILL.md is the sole authored workflow. This script
// deterministically publishes docs/skill.md from it, rewriting only the
// package-relative reference links to their public documentation URLs, so any
// other drift between the two files fails CI (--check).
export const CANONICAL_SKILL_PATH = "skills/anarlog/SKILL.md";
export const PUBLISHED_SKILL_PATH = "docs/skill.md";

export const REFERENCE_LINK_REWRITES = {
  "references/cli.md": "https://docs.anarlog.so/reference/cli",
  "references/mcp.md": "https://docs.anarlog.so/reference/mcp",
  "references/errors.md": "https://docs.anarlog.so/reference/errors",
  "references/setup.md": "https://docs.anarlog.so/agents/overview",
};

export function publishSkill(canonical) {
  let published = canonical;
  for (const [reference, url] of Object.entries(REFERENCE_LINK_REWRITES)) {
    published = published.split(`](${reference})`).join(`](${url})`);
  }

  const unrewritten = published.match(/\]\(references\/[^)]*\)/);
  if (unrewritten) {
    throw new Error(
      `SKILL.md links to ${unrewritten[0]} which has no public URL mapping`,
    );
  }
  return published;
}

function main() {
  const check = process.argv.includes("--check");
  const canonical = readFileSync(CANONICAL_SKILL_PATH, "utf8");
  const published = publishSkill(canonical);

  if (check) {
    const current = readFileSync(PUBLISHED_SKILL_PATH, "utf8");
    if (current !== published) {
      console.error(
        `${PUBLISHED_SKILL_PATH} drifted from ${CANONICAL_SKILL_PATH}; run: node scripts/publish-anarlog-skill.mjs`,
      );
      process.exitCode = 1;
      return;
    }
    console.log(`${PUBLISHED_SKILL_PATH} matches ${CANONICAL_SKILL_PATH}`);
    return;
  }

  writeFileSync(PUBLISHED_SKILL_PATH, published);
  console.log(`Published ${PUBLISHED_SKILL_PATH} from ${CANONICAL_SKILL_PATH}`);
}

const isMain = process.argv[1]
  ? import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
  : false;

if (isMain) {
  main();
}
