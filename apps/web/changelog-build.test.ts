import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  buildChangelogModule,
  getPublishedDesktopReleases,
  renderChangelogModule,
} from "./changelog-build.ts";
import { getMobilePublication } from "./changelog-publication.ts";

const desktopPublication = {
  publishedAt: "2026-09-08T11:12:17Z",
  channel: "stable" as const,
  availability: [],
};

const published = {
  tag_name: "desktop_v1.4.23",
  draft: false,
  prerelease: false,
  published_at: "2026-09-08T11:12:17Z",
};

test("accepts only actually published stable desktop releases", async () => {
  const versions = await getPublishedDesktopReleases(async () =>
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
  assert.deepEqual([...versions.keys()], ["desktop/1.4.23"]);
});

test("includes older releases across pages without following arbitrary URLs", async () => {
  const urls: string[] = [];
  const versions = await getPublishedDesktopReleases(async (url) => {
    urls.push(String(url));
    return urls.length === 1
      ? Response.json([published], {
          headers: { link: '<https://example.com>; rel="next"' },
        })
      : Response.json([{ ...published, tag_name: "desktop_v1.0.0" }]);
  });
  assert.deepEqual([...versions.keys()], ["desktop/1.4.23", "desktop/1.0.0"]);
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
  await getPublishedDesktopReleases(async (url, init) => {
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
  await getPublishedDesktopReleases(async (_url, init) => {
    assert.equal(new Headers(init?.headers).has("Authorization"), false);
    return Response.json([published]);
  });
});

test("does not return a partial allowlist when a later page fails", async () => {
  let requests = 0;
  await assert.rejects(
    getPublishedDesktopReleases(async () => {
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
    getPublishedDesktopReleases(
      async () => new Response(null, { status: 429 }),
    ),
    /429/,
  );
  await assert.rejects(
    getPublishedDesktopReleases(async () =>
      Response.json({ message: "invalid" }),
    ),
    /Invalid/,
  );
  await assert.rejects(
    getPublishedDesktopReleases(async () => {
      throw new Error("offline");
    }),
    /offline/,
  );
});

test("unreleased notes are absent from the website module, including its raw imports", () => {
  const files = [
    "/content/desktop/1.4.23.md",
    "/content/desktop/1.4.24.md",
    "/content/nightly.md",
    "/content/AGENTS.md",
  ];
  const module = renderChangelogModule(
    files,
    new Map([["desktop/1.4.23", desktopPublication]]),
  );
  assert.match(module, /1\.4\.23\.md\?raw/);
  assert.doesNotMatch(module, /1\.4\.24|nightly|AGENTS/);
  assert.equal(renderChangelogModule(files, new Map()), "export default {};");
  assert.match(
    renderChangelogModule(
      files,
      new Map(
        ["1.4.23", "1.4.24"].map((version) => [
          `desktop/${version}`,
          desktopPublication,
        ]),
      ),
    ),
    /1\.4\.24\.md\?raw/,
  );
});

test("local development can preview stable drafts but never Nightly or instruction files", () => {
  const module = renderChangelogModule(
    ["/content/desktop/1.4.24.md", "/content/nightly.md", "/content/AGENTS.md"],
    null,
  );
  assert.match(module, /1\.4\.24\.md\?raw/);
  assert.doesNotMatch(module, /nightly|AGENTS/);
});

test("normalizes Windows paths in both imports and entry keys", () => {
  const module = renderChangelogModule(
    [
      "C:\\repo\\content\\desktop\\1.4.23.md",
      "C:\\repo\\content\\desktop\\1.4.24.md",
    ],
    new Map([["desktop/1.4.23", desktopPublication]]),
  );
  assert.ok(
    module.includes(
      'import entry0 from "C:/repo/content/desktop/1.4.23.md?raw";',
    ),
  );
  assert.ok(
    module.includes('"C:/repo/content/desktop/1.4.23.md": {raw: entry0'),
  );
  assert.doesNotMatch(module, /1\.4\.24|\\/);
});

const mobileRecord = {
  version: "1.4.23",
  sourceSha: "a".repeat(40),
  channel: "beta",
  availability: [
    {
      platform: "ios",
      destination: "testflight",
      build: "42",
      publishedAt: "2026-09-09T00:00:00Z",
      evidenceUrl: "https://appstoreconnect.apple.com/apps/123/testflight",
    },
  ],
};

test("mobile publication requires verified availability for the matching version and channel", () => {
  const publication = getMobilePublication(
    JSON.stringify(mobileRecord),
    "1.4.23",
  );
  assert.equal(publication.channel, "beta");
  assert.deepEqual(publication.availability, [
    { platform: "ios", destination: "testflight" },
  ]);
  for (const record of [
    { ...mobileRecord, availability: [] },
    { ...mobileRecord, sourceSha: "unknown" },
    { ...mobileRecord, version: "1.4.24" },
    { ...mobileRecord, channel: "stable" },
    {
      ...mobileRecord,
      availability: [
        mobileRecord.availability[0],
        mobileRecord.availability[0],
      ],
    },
    {
      ...mobileRecord,
      availability: [
        {
          ...mobileRecord.availability[0],
          publishedAt: "2999-01-01T00:00:00Z",
        },
      ],
    },
    {
      ...mobileRecord,
      availability: [
        { ...mobileRecord.availability[0], destination: "play-beta" },
      ],
    },
  ])
    assert.throws(() => getMobilePublication(JSON.stringify(record), "1.4.23"));
});

test("website builds publish streams independently and keep unavailable mobile notes out of bundles", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "anarlog-changelog-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  t.mock.method(globalThis, "fetch", async () => Response.json([published]));
  for (const stream of ["desktop", "mobile"]) {
    await mkdir(join(directory, stream));
    for (const version of ["1.4.23", "1.4.24"]) {
      await writeFile(
        join(directory, stream, `${version}.md`),
        `Notes for ${stream} ${version}`,
      );
    }
  }
  await writeFile(
    join(directory, "mobile", "1.4.23.json"),
    JSON.stringify(mobileRecord),
  );
  await writeFile(join(directory, "1.4.23.md"), "Legacy copy");
  const module = await buildChangelogModule("build", directory);
  assert.match(module, /desktop\/1\.4\.23\.md\?raw/);
  assert.match(module, /mobile\/1\.4\.23\.md\?raw/);
  assert.doesNotMatch(module, /1\.4\.24|sourceSha|evidenceUrl|Legacy/);
  assert.match(module, /testflight/);
  const preview = await buildChangelogModule("serve", directory);
  assert.match(preview, /desktop\/1\.4\.24\.md\?raw/);
  assert.match(preview, /mobile\/1\.4\.24\.md\?raw/);
  await writeFile(
    join(directory, "mobile", "1.4.24.json"),
    JSON.stringify(mobileRecord),
  );
  await assert.rejects(
    buildChangelogModule("build", directory),
    /does not match/,
  );
});
