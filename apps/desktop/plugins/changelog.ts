import { readdirSync, readFileSync, watch } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin, ViteDevServer } from "vite";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const changelogDir = resolve(__dirname, "../../../packages/changelog/content");

const VIRTUAL_ID = "virtual:changelog";
const RESOLVED_ID = "\0" + VIRTUAL_ID;

function getLatestVersion(): string | null {
  try {
    const files = readdirSync(changelogDir).filter(
      (f) => f.endsWith(".md") && /^\d/.test(f),
    );
    const versions = files
      .map((f) => f.replace(".md", ""))
      .filter((v) => !v.includes("nightly"));
    versions.sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
    return versions[0] || null;
  } catch {
    return null;
  }
}

function buildModule(): string {
  const latest = getLatestVersion();
  let content: string | null = null;

  if (latest) {
    try {
      content = readFileSync(resolve(changelogDir, `${latest}.md`), "utf-8");
    } catch {}
  }

  return [
    `export const latestVersion = ${JSON.stringify(latest)};`,
    `export const latestContent = ${JSON.stringify(content)};`,
  ].join("\n");
}

export function changelog(): Plugin {
  return {
    name: "changelog",
    resolveId(id) {
      if (id === VIRTUAL_ID) return RESOLVED_ID;
    },
    load(id) {
      if (id === RESOLVED_ID) return buildModule();
    },
    configureServer(server: ViteDevServer) {
      try {
        watch(changelogDir, { recursive: true }, () => {
          const mod = server.moduleGraph.getModuleById(RESOLVED_ID);
          if (mod) {
            server.moduleGraph.invalidateModule(mod);
            server.ws.send({ type: "full-reload" });
          }
        });
      } catch {}
    },
  };
}
