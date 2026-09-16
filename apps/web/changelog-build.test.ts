import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { isAbsolute, join, relative } from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";

import {
  buildChangelogModule,
  getPublishedDesktopVersions,
  publishedChangelogs,
  renderChangelogModule,
} from "./changelog-build.ts";

const published = {
  tag_name: "desktop_v1.4.23",
  draft: false,
  prerelease: false,
  published_at: "2026-09-08T11:12:17Z",
};

test("accepts only actually published stable desktop releases", async () => {
  const versions = await getPublishedDesktopVersions(async () =>
    Response.json([
      published,
      { ...published, tag_name: "desktop_v1.4.24", draft: true },
      { ...published, tag_name: "desktop_v1.4.25", prerelease: true },
      { ...published, tag_name: "desktop_v1.4.26", published_at: null },
      { ...published, tag_name: "desktop_v1.4.27", published_at: "invalid" },
      { ...published, tag_name: "desktop_nightly_v1.4.24-nightly.4" },
      { ...published, tag_name: "cli_v1.4.24" },
      { tag_name: "desktop_v1.4.28" },
      null,
    ]),
  );
  assert.deepEqual([...versions], ["1.4.23"]);
});

test("includes older releases across pages without following arbitrary URLs", async () => {
  const urls: string[] = [];
  const versions = await getPublishedDesktopVersions(async (url) => {
    urls.push(String(url));
    return urls.length === 1
      ? Response.json([published], {
          headers: { link: '<https://example.com>; rel="next"' },
        })
      : Response.json([{ ...published, tag_name: "desktop_v1.0.0" }]);
  });
  assert.deepEqual([...versions], ["1.4.23", "1.0.0"]);
  assert.deepEqual(
    urls,
    [1, 2].map(
      (page) =>
        `https://api.github.com/repos/fastrepl/anarlog/releases?per_page=100&page=${page}`,
    ),
  );
});

test("uses build-only authentication on every page and rejects redirects", async () => {
  let requests = 0;
  await getPublishedDesktopVersions(async (url, init) => {
    requests++;
    assert.equal(new URL(url).origin, "https://api.github.com");
    assert.equal(
      new Headers(init?.headers).get("Authorization"),
      "Bearer test-build-token",
    );
    assert.equal(init?.redirect, "error");
    return Response.json(
      [published],
      requests === 1
        ? { headers: { link: '<https://example.com>; rel="next"' } }
        : undefined,
    );
  }, "test-build-token");
  assert.equal(requests, 2);
  await getPublishedDesktopVersions(async (_url, init) => {
    assert.equal(new Headers(init?.headers).has("Authorization"), false);
    return Response.json([published]);
  });
});

test("does not return a partial allowlist when a later page fails", async () => {
  let requests = 0;
  await assert.rejects(
    getPublishedDesktopVersions(async () => {
      return ++requests === 1
        ? Response.json([published], {
            headers: { link: '<https://example.com>; rel="next"' },
          })
        : new Response(null, { status: 403 });
    }),
    /403/,
  );
});

test("fails the build rather than exposing drafts when publication cannot be checked", async () => {
  await assert.rejects(
    getPublishedDesktopVersions(
      async () => new Response(null, { status: 429 }),
    ),
    /429/,
  );
  await assert.rejects(
    getPublishedDesktopVersions(async () =>
      Response.json({ message: "invalid" }),
    ),
    /Invalid/,
  );
  await assert.rejects(
    getPublishedDesktopVersions(async () => {
      throw new Error("offline");
    }),
    /offline/,
  );
});

test("unreleased notes are absent from the website module, including its raw imports", () => {
  const files = [
    "/content/1.4.23.md",
    "/content/1.4.24.md",
    "/content/nightly.md",
    "/content/AGENTS.md",
  ];
  const module = renderChangelogModule(files, new Set(["1.4.23"]));
  assert.match(module, /1\.4\.23\.md\?raw/);
  assert.doesNotMatch(module, /1\.4\.24|nightly|AGENTS/);
  assert.equal(renderChangelogModule(files, new Set()), "export default {};");
  assert.match(
    renderChangelogModule(files, new Set(["1.4.23", "1.4.24"])),
    /1\.4\.24\.md\?raw/,
  );
});

test("local development can preview stable drafts but never Nightly or instruction files", () => {
  const module = renderChangelogModule(
    ["/content/1.4.24.md", "/content/nightly.md", "/content/AGENTS.md"],
    null,
  );
  assert.match(module, /1\.4\.24\.md\?raw/);
  assert.doesNotMatch(module, /nightly|AGENTS/);
});

test("normalizes Windows paths in both imports and entry keys", () => {
  const module = renderChangelogModule(
    ["C:\\repo\\content\\1.4.23.md", "C:\\repo\\content\\1.4.24.md"],
    new Set(["1.4.23"]),
  );
  assert.ok(
    module.includes('import entry0 from "C:/repo/content/1.4.23.md?raw";'),
  );
  assert.ok(module.includes('"C:/repo/content/1.4.23.md": entry0'));
  assert.doesNotMatch(module, /1\.4\.24|\\/);
});

test(
  "relative-directory dev preview discovers added and deleted notes without restarting",
  { timeout: 15_000 },
  async (t) => {
    const directory = await mkdtemp(join(process.cwd(), ".changelog-hmr-"));
    t.after(() => rm(directory, { recursive: true, force: true }));
    await writeFile(join(directory, "1.4.23.md"), "Released note");
    const inputDirectory = relative(process.cwd(), directory);
    assert.ok(!isAbsolute(inputDirectory));
    assert.equal(
      await buildChangelogModule("serve", inputDirectory),
      await buildChangelogModule("serve", directory),
    );
    const { createServer } = await import("vite");
    const server = await createServer({
      root: directory,
      configFile: false,
      publicDir: false,
      plugins: [await publishedChangelogs("serve", inputDirectory)],
      optimizeDeps: { noDiscovery: true, include: [] },
      server: { host: "127.0.0.1", port: 0 },
      logLevel: "silent",
    });
    t.after(() => server.close());
    await server.listen();
    const url = "virtual:published-changelogs";
    assert.match((await server.transformRequest(url))!.code, /1\.4\.23/);
    for (
      let tries = 0;
      !Object.values(server.watcher.getWatched()).some((files) =>
        files.includes("1.4.23.md"),
      );
      tries++
    ) {
      assert.ok(tries < 250, "content directory is watched");
      await delay(20);
    }
    let reloads = 0;
    t.mock.method(
      server.environments.client.hot,
      "send",
      (payload: { type: string }) => {
        if (payload.type === "full-reload") reloads++;
      },
    );
    const draft = join(directory, "1.4.24.md");
    await writeFile(draft, "Draft note");
    for (let tries = 0; reloads < 1; tries++) {
      assert.ok(tries < 250, "adding a note triggers reload");
      await delay(20);
    }
    assert.match((await server.transformRequest(url))!.code, /1\.4\.24/);
    const previousReloads = reloads;
    await rm(draft);
    for (let tries = 0; reloads === previousReloads; tries++) {
      assert.ok(tries < 250, "deleting a note triggers reload");
      await delay(20);
    }
    assert.doesNotMatch((await server.transformRequest(url))!.code, /1\.4\.24/);
  },
);
