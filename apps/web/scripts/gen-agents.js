import { readdir, writeFile } from "fs/promises";
import { join, relative } from "path";
import { dirname } from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = join(__dirname, "../../..");
const outputPath = join(__dirname, "../content/docs/developers/10.agents.mdx");

const GITHUB_REPO = "fastrepl/char";
const GITHUB_BASE_URL = `https://github.com/${GITHUB_REPO}/blob/main`;

const SKIP_DIRS = new Set(["node_modules", "target", "dist"]);

async function findAgentsFiles(dir, baseDir = dir, results = []) {
  try {
    const entries = await readdir(dir, { withFileTypes: true });

    for (const entry of entries) {
      if (entry.isDirectory() && SKIP_DIRS.has(entry.name)) {
        continue;
      }

      const fullPath = join(dir, entry.name);

      if (entry.isDirectory()) {
        await findAgentsFiles(fullPath, baseDir, results);
      } else if (entry.name === "AGENTS.md") {
        const relativePath = relative(baseDir, fullPath);
        results.push(relativePath);
      }
    }
  } catch {
    // Ignore errors (e.g., permission denied)
  }

  return results;
}

async function main() {
  const agentsFiles = await findAgentsFiles(repoRoot);
  agentsFiles.sort();

  let content = `---
title: "AGENTS.md"
section: "Developers"
description: "List of all AGENTS.md files in the repository"
---

| Path | Link |
|------|------|
`;

  for (const filePath of agentsFiles) {
    const githubLink = `${GITHUB_BASE_URL}/${filePath}`;
    content += `| \`${filePath}\` | [View](${githubLink}) |\n`;
  }

  await writeFile(outputPath, content, "utf-8");
  console.log(`Generated ${outputPath}`);
}

main().catch(console.error);
