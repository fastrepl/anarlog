import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

// skills/anarlog/SKILL.md is the sole authored workflow. This script
// deterministically publishes docs/skill.md from it, rewriting only the
// package-relative reference links to their public documentation URLs, so any
// other drift between the two files fails CI (--check).
export const CANONICAL_SKILL_PATH = "skills/anarlog/SKILL.md";
export const PUBLISHED_SKILL_PATH = "docs/skill.md";
export const PLUGIN_PACKAGE_MIRRORS = [
  ["skills/anarlog/SKILL.md", "agent-plugins/anarlog/skills/anarlog/SKILL.md"],
  [
    "skills/anarlog/references/cli.md",
    "agent-plugins/anarlog/skills/anarlog/references/cli.md",
  ],
  [
    "skills/anarlog/references/errors.md",
    "agent-plugins/anarlog/skills/anarlog/references/errors.md",
  ],
  [
    "skills/anarlog/references/mcp.md",
    "agent-plugins/anarlog/skills/anarlog/references/mcp.md",
  ],
  [
    "skills/anarlog/references/setup.md",
    "agent-plugins/anarlog/skills/anarlog/references/setup.md",
  ],
  ["LICENSE", "agent-plugins/anarlog/LICENSE"],
];

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
    const drifted = [];
    if (readFileIfPresent(PUBLISHED_SKILL_PATH) !== published) {
      drifted.push(PUBLISHED_SKILL_PATH);
    }
    for (const [source, target] of PLUGIN_PACKAGE_MIRRORS) {
      if (readFileIfPresent(target) !== readFileSync(source, "utf8")) {
        drifted.push(target);
      }
    }

    if (drifted.length > 0) {
      console.error(
        `${drifted.join(", ")} drifted from the Anarlog skill package; run: node scripts/publish-anarlog-skill.mjs`,
      );
      process.exitCode = 1;
      return;
    }
    console.log("Published Anarlog skill files are current");
    return;
  }

  writeFileSync(PUBLISHED_SKILL_PATH, published);
  for (const [source, target] of PLUGIN_PACKAGE_MIRRORS) {
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, readFileSync(source, "utf8"));
  }
  console.log("Published Anarlog skill files");
}

function readFileIfPresent(filePath) {
  try {
    return readFileSync(filePath, "utf8");
  } catch (error) {
    if (error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

const isMain = process.argv[1]
  ? import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
  : false;

if (isMain) {
  main();
}
