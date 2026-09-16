import { readdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

import { getChangelogVersionFromPath } from "./src/lib/changelog-path.ts";

const contentDirectory = fileURLToPath(
  new URL("../../packages/changelog/content/", import.meta.url),
);

export async function getPublishedDesktopVersions(
  request: (input: string, init?: RequestInit) => Promise<Response> = fetch,
  token?: string,
) {
  const versions = new Set<string>();

  for (let page = 1; ; page++) {
    const response = await request(
      `https://api.github.com/repos/fastrepl/anarlog/releases?per_page=100&page=${page}`,
      {
        headers: {
          Accept: "application/vnd.github+json",
          "X-GitHub-Api-Version": "2022-11-28",
          "User-Agent": "Anarlog-Changelog-Build",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        redirect: "error",
        signal: AbortSignal.timeout(15_000),
      },
    );
    if (!response.ok) {
      throw new Error(
        `Cannot verify published desktop releases: ${response.status}`,
      );
    }
    const releases: unknown = await response.json();
    if (!Array.isArray(releases)) {
      throw new Error("Invalid published desktop releases response");
    }

    for (const release of releases) {
      if (
        !release ||
        release.draft !== false ||
        release.prerelease !== false ||
        typeof release.published_at !== "string" ||
        !Number.isFinite(Date.parse(release.published_at)) ||
        typeof release.tag_name !== "string"
      )
        continue;
      const version = /^desktop_v(\d+\.\d+\.\d+)$/.exec(release.tag_name)?.[1];
      if (version) versions.add(version);
    }

    if (!response.headers.get("link")?.includes('rel="next"')) return versions;
  }
}

export function renderChangelogModule(
  files: string[],
  publishedVersions: ReadonlySet<string> | null,
) {
  const paths = files
    .map((path) => path.replaceAll("\\", "/"))
    .filter((path) => {
      const version = getChangelogVersionFromPath(path);
      return (
        version &&
        (publishedVersions === null || publishedVersions.has(version))
      );
    });
  return [
    ...paths.map(
      (path, index) =>
        `import entry${index} from ${JSON.stringify(`${path}?raw`)};`,
    ),
    `export default {${paths.map((path, index) => `${JSON.stringify(path)}: entry${index}`).join(",")}};`,
  ].join("\n");
}

export async function buildChangelogModule(
  command: "serve" | "build",
  directory = contentDirectory,
) {
  directory = resolve(directory);
  const files = (await readdir(directory))
    .sort()
    .map((file) => join(directory, file));
  // Local editing may preview drafts; every deployable build requires publication evidence.
  const publishedVersions =
    command === "serve"
      ? null
      : await getPublishedDesktopVersions(fetch, process.env.GITHUB_TOKEN);
  return renderChangelogModule(files, publishedVersions);
}

export async function publishedChangelogs(
  command: "serve" | "build",
  directory = contentDirectory,
): Promise<Plugin> {
  directory = resolve(directory);
  const moduleId = "\0virtual:published-changelogs";
  const builtModule =
    command === "build" ? await buildChangelogModule(command, directory) : null;
  const normalizedDirectory = directory
    .replaceAll("\\", "/")
    .replace(/\/$/, "");

  return {
    name: "published-changelogs",
    resolveId(id) {
      if (id === "virtual:published-changelogs") return moduleId;
    },
    load(id) {
      if (id === moduleId)
        return builtModule ?? buildChangelogModule("serve", directory);
    },
    configureServer(server) {
      server.watcher.add(directory);
    },
    hotUpdate({ file, type }) {
      const path = file.replaceAll("\\", "/");
      if (
        type === "update" ||
        dirname(path) !== normalizedDirectory ||
        !getChangelogVersionFromPath(path)
      )
        return;
      const module = this.environment.moduleGraph.getModuleById(moduleId);
      if (!module) return;
      this.environment.moduleGraph.invalidateModule(module);
      this.environment.hot.send({ type: "full-reload" });
      return [];
    },
  };
}
