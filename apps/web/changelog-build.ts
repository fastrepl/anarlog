import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

import {
  getMobilePublication,
  type ChangelogPublication,
} from "./changelog-publication.ts";
import {
  getChangelogReleaseFromPath,
  changelogStreams,
} from "./src/lib/changelog-path.ts";

const contentDirectory = fileURLToPath(
  new URL("../../packages/changelog/content/", import.meta.url),
);

export async function getPublishedDesktopReleases(
  request: (input: string, init?: RequestInit) => Promise<Response> = fetch,
  token?: string,
) {
  const versions = new Map<string, ChangelogPublication>();

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
      if (version)
        versions.set(`desktop/${version}`, {
          publishedAt: release.published_at,
          channel: "stable",
          availability: [],
        });
    }

    if (!response.headers.get("link")?.includes('rel="next"')) return versions;
  }
}

export function renderChangelogModule(
  files: string[],
  publications: ReadonlyMap<string, ChangelogPublication> | null,
  preview = false,
) {
  const paths = files
    .map((path) => path.replaceAll("\\", "/"))
    .filter((path) => {
      const release = getChangelogReleaseFromPath(path);
      return (
        release &&
        (preview ||
          publications === null ||
          publications.has(`${release.stream}/${release.version}`))
      );
    });
  return [
    ...paths.map(
      (path, index) =>
        `import entry${index} from ${JSON.stringify(`${path}?raw`)};`,
    ),
    `export default {${paths
      .map((path, index) => {
        const release = getChangelogReleaseFromPath(path)!;
        const publication =
          publications?.get(`${release.stream}/${release.version}`) ?? null;
        return `${JSON.stringify(path)}: {raw: entry${index}, publication: ${JSON.stringify(publication)}}`;
      })
      .join(",")}};`,
  ].join("\n");
}

export async function buildChangelogModule(
  command: "serve" | "build",
  directory = contentDirectory,
) {
  directory = resolve(directory);
  const files = (
    await Promise.all(
      changelogStreams.map(async (stream) =>
        (await readdir(join(directory, stream)))
          .sort()
          .map((file) => join(directory, stream, file)),
      ),
    )
  ).flat();
  const publications =
    command === "serve"
      ? new Map<string, ChangelogPublication>()
      : await getPublishedDesktopReleases(fetch, process.env.GITHUB_TOKEN);
  for (const file of files) {
    const release = getChangelogReleaseFromPath(file);
    if (release?.stream !== "mobile") continue;
    const record = file.replace(/\.md$/, ".json");
    let raw: string;
    try {
      raw = await readFile(record, "utf8");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") continue;
      throw error;
    }
    publications.set(
      `mobile/${release.version}`,
      getMobilePublication(raw, release.version),
    );
  }
  return renderChangelogModule(files, publications, command === "serve");
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
    hotUpdate({ file }) {
      const path = file.replaceAll("\\", "/");
      if (
        !path.startsWith(`${normalizedDirectory}/`) ||
        !getChangelogReleaseFromPath(path.replace(/\.json$/, ".md"))
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
