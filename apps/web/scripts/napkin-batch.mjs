#!/usr/bin/env node
import { spawn } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const ARTICLES_DIR = resolve(webDir, "content/articles");
const MANIFEST_PATH = resolve(ARTICLES_DIR, "figures.json");
const SINGLE_SCRIPT = resolve(scriptDir, "napkin-to-supabase.mjs");

// Mirrors STORAGE_BUCKETS.blog in src/routes/api/assets.$.ts. Public, so the
// existence check needs no credentials and is safe to run in CI.
const PUBLIC_BLOG_BASE =
  "https://auth.hyprnote.com/storage/v1/object/public/blog";

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) throw new Error(`Unexpected argument: ${arg}`);
    const [rawKey, inline] = arg.slice(2).split("=", 2);
    const key = rawKey.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (inline !== undefined) {
      args[key] = inline;
      continue;
    }
    const next = argv[i + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
      continue;
    }
    args[key] = next;
    i += 1;
  }
  return args;
}

function usage() {
  return `Usage:
  pnpm -F @anlg/web media:figures            Generate every declared figure that is missing
  pnpm -F @anlg/web media:figures:check      Report missing figures, generate nothing

Options:
  --check          Report only, generate nothing. Needs no credentials, so it is
                   safe in CI. Exits 1 when an article has no manifest entry or
                   the manifest names a slug with no article. A declared but
                   not-yet-generated figure only warns.
  --strict         With --check, also exit 1 on not-yet-generated figures.
  --slug <slug>    Limit to one article.
  --upsert         Regenerate and replace figures that already exist.
  --dry-run        Print what each generation would request.

Declare figures in content/articles/figures.json:

  {
    "my-post-slug": [
      {
        "filename": "capture-flow.png",
        "content": "Meeting audio\\nLocal transcription\\nMarkdown note",
        "context": "Horizontal flow diagram for an Anarlog blog post.",
        "visualQuery": "flowchart",
        "orientation": "horizontal",
        "width": 1200
      }
    ]
  }

An empty array marks a post as deliberately figure-less and satisfies --check.

Generation requires NAPKIN_API_TOKEN, SUPABASE_URL, and SUPABASE_SERVICE_ROLE_KEY;
see napkin-to-supabase.md for the Infisical command.
`;
}

async function readArticleSlugs() {
  const entries = await readdir(ARTICLES_DIR);
  return entries
    .filter((f) => f.endsWith(".mdx"))
    .map((f) => f.replace(/\.mdx$/, ""))
    .sort();
}

async function readManifest() {
  let raw;
  try {
    raw = await readFile(MANIFEST_PATH, "utf8");
  } catch (error) {
    if (error.code === "ENOENT") return {};
    throw error;
  }
  const parsed = JSON.parse(raw);
  if (Array.isArray(parsed) || typeof parsed !== "object" || parsed === null) {
    throw new Error("figures.json must be an object keyed by article slug");
  }
  return parsed;
}

function storagePath(slug, filename) {
  return `articles/${slug}/${filename}`;
}

async function figureExists(slug, filename) {
  const url = `${PUBLIC_BLOG_BASE}/${storagePath(slug, filename)}`;
  try {
    const response = await fetch(url, { method: "HEAD" });
    return response.ok;
  } catch {
    return false;
  }
}

function runSingle(slug, figure, { upsert, dryRun }) {
  const args = [
    SINGLE_SCRIPT,
    "--slug",
    slug,
    "--filename",
    figure.filename,
    "--content",
    figure.content,
  ];

  const passthrough = {
    context: "--context",
    visualQuery: "--visual-query",
    orientation: "--orientation",
    width: "--width",
    height: "--height",
    format: "--format",
    styleId: "--style-id",
    language: "--language",
    numberOfVisuals: "--number-of-visuals",
    fileIndex: "--file-index",
    colorMode: "--color-mode",
  };

  for (const [key, flag] of Object.entries(passthrough)) {
    if (figure[key] !== undefined) args.push(flag, String(figure[key]));
  }
  if (figure.transparentBackground) args.push("--transparent-background");
  if (upsert) args.push("--upsert");
  if (dryRun) args.push("--dry-run");

  return new Promise((resolvePromise, reject) => {
    const child = spawn(process.execPath, args, {
      cwd: webDir,
      stdio: "inherit",
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`napkin-to-supabase exited with code ${code}`));
    });
  });
}

function validateFigure(slug, figure, index) {
  const where = `${slug}[${index}]`;
  if (!figure || typeof figure !== "object") {
    throw new Error(`${where} must be an object`);
  }
  if (!figure.filename) throw new Error(`${where} is missing "filename"`);
  if (!figure.content) throw new Error(`${where} is missing "content"`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }

  const checkOnly = Boolean(args.check);
  const onlySlug = typeof args.slug === "string" ? args.slug : null;

  const [slugs, manifest] = await Promise.all([
    readArticleSlugs(),
    readManifest(),
  ]);

  const undeclared = slugs.filter(
    (slug) => !Object.prototype.hasOwnProperty.call(manifest, slug),
  );
  const orphaned = Object.keys(manifest).filter(
    (slug) => !slugs.includes(slug),
  );

  const targets = [];
  for (const [slug, figures] of Object.entries(manifest)) {
    if (onlySlug && slug !== onlySlug) continue;
    if (!Array.isArray(figures)) {
      throw new Error(`figures.json: "${slug}" must map to an array`);
    }
    figures.forEach((figure, index) => {
      validateFigure(slug, figure, index);
      targets.push({ slug, figure });
    });
  }

  const missing = [];
  for (const target of targets) {
    const exists = await figureExists(target.slug, target.figure.filename);
    if (!exists || args.upsert) missing.push(target);
  }

  if (orphaned.length > 0) {
    console.error(
      `figures.json declares ${orphaned.length} slug(s) with no matching article:`,
    );
    for (const slug of orphaned) console.error(`  - ${slug}`);
  }

  if (checkOnly) {
    // Undeclared articles and stale entries fail the build: those are authoring
    // mistakes. A generation backlog only warns unless --strict, so an unrun
    // batch never turns CI red on its own.
    const strict = Boolean(args.strict);

    if (undeclared.length > 0) {
      console.error(
        `\n${undeclared.length} article(s) not declared in figures.json:`,
      );
      for (const slug of undeclared) console.error(`  - ${slug}`);
      console.error(
        "\nAdd an entry for each. Use [] to mark a post as deliberately figure-less.",
      );
    }
    if (missing.length > 0) {
      console.error(
        `\n${missing.length} declared figure(s) not yet uploaded${strict ? "" : " (warning)"}:`,
      );
      for (const { slug, figure } of missing) {
        console.error(`  - ${storagePath(slug, figure.filename)}`);
      }
      console.error(
        "\nRun `pnpm -F @anlg/web media:figures` with Napkin and Supabase credentials.",
      );
    }

    const failed =
      undeclared.length > 0 ||
      orphaned.length > 0 ||
      (strict && missing.length > 0);

    if (!failed && missing.length === 0) {
      console.log(
        `All ${targets.length} declared figure(s) present across ${slugs.length} article(s).`,
      );
    }
    if (failed) process.exit(1);
    return;
  }

  if (undeclared.length > 0) {
    console.error(
      `Note: ${undeclared.length} article(s) have no figures.json entry and will be skipped.`,
    );
  }

  if (missing.length === 0) {
    console.log("Nothing to generate; every declared figure already exists.");
    return;
  }

  console.error(`Generating ${missing.length} figure(s)...\n`);

  const failures = [];
  for (const [index, { slug, figure }] of missing.entries()) {
    const label = storagePath(slug, figure.filename);
    console.error(`[${index + 1}/${missing.length}] ${label}`);
    try {
      await runSingle(slug, figure, {
        upsert: Boolean(args.upsert),
        dryRun: Boolean(args.dryRun),
      });
    } catch (error) {
      failures.push({ label, message: error.message });
      console.error(`  failed: ${error.message}`);
    }
  }

  if (failures.length > 0) {
    console.error(`\n${failures.length} figure(s) failed:`);
    for (const failure of failures) {
      console.error(`  - ${failure.label}: ${failure.message}`);
    }
    process.exit(1);
  }

  console.error(`\nGenerated ${missing.length} figure(s).`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
