import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createManifest,
  releasePlatformPlan,
  verifyDesktopPlatformSets,
  verifyLocalAssets,
  verifyManifest,
  verifyWorkflowPlatformCoverage,
} from "./desktop-release-provenance.mjs";

const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const cnAssetId = "01KVDB8KPSKMQ5X3SJ0ANF6943";
const cnSha256 =
  "760b11d1ab9326dc78068ac8ef450685ea116e329903b14d94a5133641a54128";
const cnVersion = "cn 0.13.2";

function createDesktopRelease({
  includeLinux = true,
  includeWindows = true,
  splitMappings = false,
} = {}) {
  const platformPairs = [
    ["dmg-aarch64", "darwin-aarch64", true],
    ["dmg-x86_64", "darwin-x86_64", true],
    ...(includeLinux
      ? [
          ["appimage-x86_64", "linux-x86_64-appimage", false],
          ["debian-x86_64", "linux-x86_64-deb", false],
          ["appimage-aarch64", "linux-aarch64-appimage", false],
          ["debian-aarch64", "linux-aarch64-deb", false],
        ]
      : []),
    ...(includeWindows ? [["nsis-x86_64", "windows-x86_64-nsis", false]] : []),
  ];

  return {
    version: "1.4.0",
    status: "draft",
    assets: platformPairs.flatMap(
      ([publicPlatform, updatePlatform, separateAssets], index) =>
        splitMappings || separateAssets
          ? [
              {
                id: `asset-public-${index}`,
                publicPlatform,
                updatePlatform: null,
                size: index + 1,
                signature: null,
              },
              {
                id: `asset-update-${index}`,
                publicPlatform: null,
                updatePlatform,
                size: index + 1,
                signature: `signature-${index}`,
              },
            ]
          : [
              {
                id: `asset-${index}`,
                publicPlatform,
                updatePlatform,
                size: index + 1,
                signature: `signature-${index}`,
              },
            ],
    ),
  };
}

test("accepts each supported desktop platform selection", () => {
  for (const [releaseOptions, verifyOptions] of [
    [{}, undefined],
    [{ splitMappings: true }, undefined],
    [{ includeWindows: false }, { includeWindows: false }],
    [
      { includeLinux: false, includeWindows: false },
      { includeLinux: false, includeWindows: false },
    ],
  ]) {
    verifyDesktopPlatformSets(
      createDesktopRelease(releaseOptions),
      verifyOptions,
    );
  }
});

test("rejects incomplete, extra, duplicate, opaque, or unsigned platform assets", () => {
  const extra = (asset) => (release) => {
    release.assets.push({ id: "asset-extra", size: 1, ...asset });
  };
  for (const [mutate, error, releaseOptions] of [
    [() => {}, /public platforms do not match/, { includeWindows: false }],
    [
      extra({
        publicPlatform: "rpm-x86_64",
        updatePlatform: null,
        signature: null,
      }),
      /public platforms do not match/,
    ],
    [
      (release) => {
        release.assets[0].publicPlatform = release.assets[2].publicPlatform;
      },
      /public platforms do not match/,
    ],
    [
      extra({
        publicPlatform: null,
        updatePlatform: "linux-x86_64-rpm",
        signature: "signature-extra",
      }),
      /update platforms do not match/,
    ],
    [
      extra({ publicPlatform: null, updatePlatform: null, signature: null }),
      /must map to a public or update platform/,
    ],
    [
      (release) => {
        release.assets.find(
          (asset) => asset.updatePlatform !== null,
        ).signature = null;
      },
      /updater asset must carry a signature/,
    ],
  ]) {
    const release = createDesktopRelease(releaseOptions);
    mutate(release);
    assert.throws(() => verifyDesktopPlatformSets(release), error);
  }
});

function workflowFixtures({ publicPlatforms } = {}) {
  const platforms =
    publicPlatforms ??
    Object.values(releasePlatformPlan)
      .filter((group) => typeof group === "object" && group.publicPlatforms)
      .flatMap((group) => group.publicPlatforms);
  const publishWorkflow = platforms
    .map(
      (platform) =>
        `      - uses: ./.github/actions/cn_download\n        with:\n          platform: ${platform}\n`,
    )
    .join("");
  const cdWorkflow = Object.values(releasePlatformPlan)
    .filter((group) => typeof group === "object" && group.buildTargets)
    .flatMap((group) => group.buildTargets)
    .map((target) => `          - target: ${target}\n`)
    .join("");
  return { publishWorkflow, cdWorkflow };
}

test("accepts workflows that cover the release plan, including repeated downloads", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  verifyWorkflowPlatformCoverage({ publishWorkflow, cdWorkflow });
  verifyWorkflowPlatformCoverage({
    publishWorkflow: `${publishWorkflow}          platform: dmg-aarch64\n`,
    cdWorkflow,
  });
});

test("rejects workflows that drift from the release plan", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  for (const [workflows, error] of [
    [
      {
        publishWorkflow: publishWorkflow.replace(
          /^.*platform: nsis-x86_64.*\n/m,
          "",
        ),
      },
      /do not match the release plan/,
    ],
    [
      { publishWorkflow: `${publishWorkflow}          platform: msi-x86_64\n` },
      /do not match the release plan/,
    ],
    [
      {
        publishWorkflow: publishWorkflow.replace(
          "platform: debian-x86_64",
          "platform: deb-x86_64",
        ),
      },
      /do not match the release plan/,
    ],
    [
      { cdWorkflow: cdWorkflow.replace("x86_64-pc-windows-msvc", "") },
      /does not build the planned target x86_64-pc-windows-msvc/,
    ],
  ]) {
    assert.throws(
      () =>
        verifyWorkflowPlatformCoverage({
          publishWorkflow,
          cdWorkflow,
          ...workflows,
        }),
      error,
    );
  }
});

test("repository release workflows match the authored release plan", async () => {
  const [publishWorkflow, cdWorkflow] = await Promise.all([
    readFile(".github/workflows/desktop_publish.yaml", "utf8"),
    readFile(".github/workflows/desktop_cd.yaml", "utf8"),
  ]);

  verifyWorkflowPlatformCoverage({ publishWorkflow, cdWorkflow });
});

test("stable desktop releases submit only the Microsoft Store package", async () => {
  const [publishWorkflow, storeWorkflow] = await Promise.all([
    readFile(".github/workflows/desktop_publish.yaml", "utf8"),
    readFile(".github/workflows/desktop_store_publish.yaml", "utf8"),
  ]);

  assert.match(storeWorkflow, /\n  workflow_call:\n/);
  assert.match(
    publishWorkflow,
    /Stable releases must include Windows for automatic Microsoft Store submission/,
  );

  const jobStart = publishWorkflow.indexOf("\n  store-publish:\n");
  assert.notEqual(jobStart, -1, "missing store-publish job");
  const remainingWorkflow = publishWorkflow.slice(jobStart + 1);
  const nextJob = remainingWorkflow.slice(1).search(/\n  [a-z][a-z0-9-]*:\n/);
  const storePublishJob =
    nextJob === -1
      ? remainingWorkflow
      : remainingWorkflow.slice(0, nextJob + 1);

  assert.match(storePublishJob, /needs: \[parse, gh-release\]/);
  assert.match(
    storePublishJob,
    /uses: \.\/\.github\/workflows\/desktop_store_publish\.yaml/,
  );
  assert.doesNotMatch(storePublishJob, /include_macos/);
  assert.match(storePublishJob, /include_windows: true/);
  assert.match(storePublishJob, /submit_to_stores: true/);
  assert.doesNotMatch(storePublishJob, /secrets: inherit/);
  assert.doesNotMatch(storeWorkflow, /\n  mac-app-store:\n/);
  assert.doesNotMatch(storeWorkflow, /app-store-connect-submit/);
  assert.doesNotMatch(storeWorkflow, /Submitted to App Review/);
  const declaredSecrets = [
    ...storeWorkflow.matchAll(
      /^      ([A-Z0-9_]+):\n        required: false$/gm,
    ),
  ].map((match) => match[1]);
  const forwardedSecrets = [
    ...storePublishJob.matchAll(
      /^      ([A-Z0-9_]+): \$\{\{ secrets\.([A-Z0-9_]+) \}\}$/gm,
    ),
  ].map((match) => {
    assert.equal(match[1], match[2]);
    return match[1];
  });

  assert.deepEqual(declaredSecrets, [
    "AZURE_AD_APPLICATION_SECRET",
    "CN_API_KEY",
    "SELLER_ID",
  ]);
  assert.deepEqual(forwardedSecrets, declaredSecrets);
});

test("desktop release workflows do not submit to the Mac App Store", async () => {
  const [desktopCi, storeWorkflow] = await Promise.all([
    readFile(".github/workflows/desktop_ci.yaml", "utf8"),
    readFile(".github/workflows/desktop_store_publish.yaml", "utf8"),
  ]);

  assert.doesNotMatch(desktopCi, /Build unsigned Mac App Store candidate/);
  assert.doesNotMatch(desktopCi, /tauri.conf.app-store.json/);
  assert.doesNotMatch(desktopCi, /anarlog-mac-app-store-unsigned/);
  assert.doesNotMatch(storeWorkflow, /\n  mac-app-store:\n/);
  assert.doesNotMatch(storeWorkflow, /include_macos/);
  assert.doesNotMatch(storeWorkflow, /app-store-connect-submit/);
  assert.doesNotMatch(storeWorkflow, /MAC_APP_STORE_/);
  assert.doesNotMatch(storeWorkflow, /APPSTORE_/);
});

const provenance = {
  version: "1.4.0",
  candidateSha,
  workflowRunId: "12345",
  cnVersion,
  cnAssetId,
  cnSha256,
};

async function createManifestFixture(t, contents) {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-release-provenance-"),
  );
  t.after(() => rm(directory, { recursive: true, force: true }));
  const assetDir = path.join(directory, "assets");
  await mkdir(assetDir);
  for (const [id, content] of Object.entries(contents)) {
    await writeFile(path.join(assetDir, id), content);
  }
  const size = (id) => Buffer.byteLength(contents[id]);
  const release = {
    version: "1.4.0",
    status: "draft",
    assets: [
      ...("asset-c" in contents
        ? [
            {
              id: "asset-c",
              publicPlatform: "appimage-x86_64",
              updatePlatform: "linux-x86_64-appimage",
              size: size("asset-c"),
              signature: "linux-signature",
            },
          ]
        : []),
      {
        id: "asset-a",
        publicPlatform: "dmg-aarch64",
        size: size("asset-a"),
      },
      {
        id: "asset-b",
        publicPlatform: "nsis-x86_64",
        updatePlatform: "windows-x86_64-nsis",
        size: size("asset-b"),
        signature: "windows-signature",
      },
    ],
  };
  const output = path.join(directory, "manifest.json");
  await createManifest({ ...provenance, release, output, assetDir });
  const manifest = JSON.parse(await readFile(output, "utf8"));
  return { assetDir, release, manifest };
}

test("binds every release asset to a candidate run and detects replacement", async (t) => {
  const { assetDir, release, manifest } = await createManifestFixture(t, {
    "asset-a": "macOS",
    "asset-b": "Windows",
    "asset-c": "Linux",
  });
  const verify = (overrides = {}) =>
    verifyManifest({
      ...provenance,
      release,
      manifest,
      assetDir,
      ...overrides,
    });

  assert.deepEqual(manifest.tools, {
    crabNebula: {
      cliVersion: cnVersion,
      cliAssetId: cnAssetId,
      cliSha256: cnSha256,
    },
  });
  assert.deepEqual(
    manifest.assets.map((asset) => asset.id),
    ["asset-a", "asset-b", "asset-c"],
  );
  await verify();

  for (const [overrides, error] of [
    [{ cnVersion: "cn 0.22.0" }, /CLI version mismatch/],
    [{ cnAssetId: "different-asset" }, /CLI asset ID mismatch/],
    [{ cnSha256: "a".repeat(64) }, /CLI SHA-256 mismatch/],
  ]) {
    await assert.rejects(verify(overrides), error);
  }

  await writeFile(path.join(assetDir, "asset-b"), "replaced");
  await assert.rejects(verify(), /size .* expected|SHA-256 changed/);
});

test("binds local GitHub release assets to exact manifest IDs and bytes", async (t) => {
  const { assetDir, manifest } = await createManifestFixture(t, {
    "asset-a": "macOS",
    "asset-b": "Windows",
  });
  const verify = (
    platformAssetIds = { "dmg-aarch64": "asset-a", "nsis-x86_64": "asset-b" },
  ) =>
    verifyLocalAssets({ ...provenance, manifest, assetDir, platformAssetIds });

  await verify();
  await assert.rejects(
    verify({ "dmg-aarch64": "asset-b", "nsis-x86_64": "asset-a" }),
    /Downloaded asset ID for dmg-aarch64 does not match the provenance manifest/,
  );

  await writeFile(path.join(assetDir, "asset-b"), "replace");
  await assert.rejects(verify(), /SHA-256 changed/);

  await writeFile(path.join(assetDir, "wrong-id"), "replace");
  await assert.rejects(
    verify(),
    /Local asset IDs do not match the provenance manifest/,
  );
});
