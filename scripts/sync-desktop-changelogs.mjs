import { readFile, readdir, writeFile } from "node:fs/promises";

const directory = new URL("../packages/changelog/content/", import.meta.url);
const check = process.argv.includes("--check");
for (const file of await readdir(new URL("desktop/", directory))) {
  if (!/^\d+\.\d+\.\d+\.md$/.test(file)) continue;
  const canonical = await readFile(
    new URL(`desktop/${file}`, directory),
    "utf8",
  );
  const legacy = new URL(file, directory);
  if (check) {
    const existing = await readFile(legacy, "utf8").catch((error) => {
      if (error.code === "ENOENT") return null;
      throw error;
    });
    if (existing !== canonical) {
      throw new Error(
        `Run node scripts/sync-desktop-changelogs.mjs to refresh ${file}`,
      );
    }
  } else {
    await writeFile(legacy, canonical);
  }
}
