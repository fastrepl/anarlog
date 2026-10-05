#!/usr/bin/env node
import { access, readdir, readFile } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const publicBlogDir = resolve(webDir, "public/images/blog");
const scanRoots = [resolve(webDir, "content/articles"), resolve(webDir, "src")];
const textExtensions = new Set([".md", ".mdx", ".ts", ".tsx"]);

const SUPABASE_BLOG_PREFIX =
  "https://ijoptyyjrfqwaqhyxkxj.supabase.co/storage/v1/object/public/blog/";
const STATIC_BLOG_PREFIX = "https://static.anarlog.so/blog/";
const legacyLocalPattern = /\/images\/blog\/[A-Za-z0-9._+%/-]+/g;
const legacyProxyPattern = /\/api\/assets\/blog\/[A-Za-z0-9._+%/-]+/g;
const supabaseBlogPattern = new RegExp(
  `${SUPABASE_BLOG_PREFIX.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[A-Za-z0-9._+%/-]+`,
  "g",
);

const staticBlogPattern = new RegExp(
  `${STATIC_BLOG_PREFIX.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[A-Za-z0-9._+%/-]+`,
  "g",
);

async function collectFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(path)));
    else files.push(path);
  }
  return files;
}

async function collectTextFiles(directory) {
  return (await collectFiles(directory)).filter(
    (file) =>
      textExtensions.has(extname(file)) && !/\.test\.[cm]?[jt]sx?$/.test(file),
  );
}

const legacyLocalReferences = [];
const legacyProxyReferences = [];
const assetUrls = new Set();
const directSupabaseReferences = [];

for (const root of scanRoots) {
  for (const file of await collectTextFiles(root)) {
    const text = await readFile(file, "utf8");
    for (const url of text.match(legacyLocalPattern) || []) {
      legacyLocalReferences.push({ file, url });
    }
    for (const url of text.match(legacyProxyPattern) || []) {
      legacyProxyReferences.push({ file, url });
    }
    for (const url of text.match(staticBlogPattern) || []) assetUrls.add(url);
    for (const url of text.match(supabaseBlogPattern) || []) {
      directSupabaseReferences.push({ file, url });
    }
  }
}

if (legacyLocalReferences.length > 0) {
  console.error(
    "Legacy /images/blog/* references remain (blog assets should be hosted on Supabase only):",
  );
  for (const item of legacyLocalReferences)
    console.error(`  - ${item.file}: ${item.url}`);
  process.exitCode = 1;
}

if (legacyProxyReferences.length > 0) {
  console.error("Legacy /api/assets/blog/* references remain:");
  for (const item of legacyProxyReferences)
    console.error(`  - ${item.file}: ${item.url}`);
  process.exitCode = 1;
}

if (directSupabaseReferences.length > 0) {
  console.error(
    "Direct Supabase blog image URLs remain; use https://static.anarlog.so/blog/:",
  );
  for (const item of directSupabaseReferences)
    console.error(`  - ${item.file}: ${item.url}`);
  process.exitCode = 1;
}

let localDirExists = true;
try {
  await access(publicBlogDir);
} catch {
  localDirExists = false;
}
if (localDirExists) {
  const leftoverFiles = await collectFiles(publicBlogDir);
  if (leftoverFiles.length > 0) {
    console.error(
      `${leftoverFiles.length} blog asset(s) still committed under public/images/blog; ` +
        "Supabase Storage should be the only host for blog assets:",
    );
    for (const file of leftoverFiles) console.error(`  - ${file}`);
    process.exitCode = 1;
  }
}

console.log(
  `Checking ${assetUrls.size} referenced public blog asset URL(s)...`,
);
const missing = [];
const urlList = [...assetUrls];
const results = [];
for (let index = 0; index < urlList.length; index += 8) {
  results.push(
    ...(await Promise.allSettled(
      urlList.slice(index, index + 8).map(async (url) => {
        let response;
        for (let attempt = 0; attempt < 3; attempt += 1) {
          try {
            response = await fetch(url, {
              method: "HEAD",
              signal: AbortSignal.timeout(30_000),
            });
            break;
          } catch (error) {
            if (attempt === 2) throw error;
          }
        }
        if (!response?.ok) throw new Error(`${response?.status}`);
      }),
    )),
  );
}
results.forEach((result, index) => {
  if (result.status === "rejected") {
    missing.push({ url: urlList[index], reason: result.reason?.message });
  }
});

if (missing.length > 0) {
  console.error(
    `${missing.length} referenced public blog asset(s) did not resolve:`,
  );
  for (const item of missing) console.error(`  - ${item.url} (${item.reason})`);
  process.exitCode = 1;
}

if (!process.exitCode) {
  console.log(
    `All ${assetUrls.size} referenced blog asset(s) resolve through their public serving URLs; ` +
      "no direct Supabase, local, or legacy-proxy image references remain in posts.",
  );
}
