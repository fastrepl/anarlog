import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createManifest,
  verifyDesktopPlatformSets,
  verifyLocalAssets,
  verifyManifest,
} from "./desktop-release-provenance.mjs";

const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const cnAssetId = "01KVDB8KPSKMQ5X3SJ0ANF6943";
const cnSha256 =
  "760b11d1ab9326dc78068ac8ef450685ea116e329903b14d94a5133641a54128";
const cnVersion = "cn 0.13.2";

function createDesktopRelease() {
  const publicPlatforms = [
    "dmg-aarch64",
    "dmg-x86_64",
    "nsis-x86_64",
    "appimage-x86_64",
    "deb-x86_64",
    "appimage-aarch64",
    "deb-aarch64",
  ];
  const updatePlatforms = [
    "darwin-aarch64",
    "darwin-x86_64",
    "windows-x86_64-nsis",
    "linux-x86_64-appimage",
    "linux-x86_64-deb",
    "linux-aarch64-appimage",
    "linux-aarch64-deb",
  ];

  return {
    version: "1.4.0",
    status: "draft",
    assets: publicPlatforms.map((publicPlatform, index) => ({
      id: `asset-${index}`,
      publicPlatform,
      updatePlatform: updatePlatforms[index],
      size: index + 1,
      signature: `signature-${index}`,
    })),
  };
}

test("accepts the exact desktop public and update platform sets", () => {
  verifyDesktopPlatformSets(createDesktopRelease());
});

test("rejects an extra public desktop platform", () => {
  const release = createDesktopRelease();
  release.assets.push({
    id: "asset-extra-public",
    publicPlatform: "rpm-x86_64",
    updatePlatform: null,
    size: 1,
    signature: null,
  });

  assert.throws(
    () => verifyDesktopPlatformSets(release),
    /exactly 7 desktop assets/,
  );
});

test("rejects duplicate public desktop platforms", () => {
  const release = createDesktopRelease();
  release.assets[0].publicPlatform = release.assets[1].publicPlatform;

  assert.throws(
    () => verifyDesktopPlatformSets(release),
    /public platforms do not match/,
  );
});

test("rejects an updater-only desktop platform", () => {
  const release = createDesktopRelease();
  release.assets.push({
    id: "asset-extra-updater",
    publicPlatform: null,
    updatePlatform: "linux-x86_64-rpm",
    size: 1,
    signature: "signature-extra",
  });

  assert.throws(
    () => verifyDesktopPlatformSets(release),
    /exactly 7 desktop assets/,
  );
});

test("rejects an opaque desktop release asset", () => {
  const release = createDesktopRelease();
  release.assets.push({
    id: "asset-opaque",
    publicPlatform: null,
    updatePlatform: null,
    size: 1,
    signature: null,
  });

  assert.throws(
    () => verifyDesktopPlatformSets(release),
    /exactly 7 desktop assets/,
  );
});

test("binds every release asset to a candidate run and detects replacement", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-release-provenance-"),
  );
  const assetDir = path.join(directory, "assets");
  await mkdir(assetDir);

  const contents = new Map([
    ["asset-a", "macOS"],
    ["asset-b", "Windows"],
    ["asset-c", "Linux"],
  ]);
  for (const [id, content] of contents) {
    await writeFile(path.join(assetDir, id), content);
  }

  const release = {
    version: "1.4.0",
    status: "draft",
    assets: [
      {
        id: "asset-c",
        publicPlatform: "appimage-x86_64",
        updatePlatform: "linux-x86_64-appimage",
        size: Buffer.byteLength(contents.get("asset-c")),
        signature: "linux-signature",
      },
      {
        id: "asset-a",
        publicPlatform: "dmg-aarch64",
        size: Buffer.byteLength(contents.get("asset-a")),
      },
      {
        id: "asset-b",
        publicPlatform: "nsis-x86_64",
        updatePlatform: "windows-x86_64-nsis",
        size: Buffer.byteLength(contents.get("asset-b")),
        signature: "windows-signature",
      },
    ],
  };
  const output = path.join(directory, "manifest.json");

  await createManifest({
    release,
    output,
    version: "1.4.0",
    candidateSha,
    workflowRunId: "12345",
    cnVersion,
    cnAssetId,
    cnSha256,
    assetDir,
  });
  const manifest = JSON.parse(await readFile(output, "utf8"));

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
  await verifyManifest({
    release,
    manifest,
    version: "1.4.0",
    candidateSha,
    workflowRunId: "12345",
    cnVersion,
    cnAssetId,
    cnSha256,
    assetDir,
  });

  await assert.rejects(
    verifyManifest({
      release,
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion: "cn 0.22.0",
      cnAssetId,
      cnSha256,
      assetDir,
    }),
    /CLI version mismatch/,
  );

  await assert.rejects(
    verifyManifest({
      release,
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId: "different-asset",
      cnSha256,
      assetDir,
    }),
    /CLI asset ID mismatch/,
  );

  await assert.rejects(
    verifyManifest({
      release,
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId,
      cnSha256: "a".repeat(64),
      assetDir,
    }),
    /CLI SHA-256 mismatch/,
  );

  await writeFile(path.join(assetDir, "asset-b"), "replaced");
  await assert.rejects(
    verifyManifest({
      release,
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId,
      cnSha256,
      assetDir,
    }),
    /size .* expected|SHA-256 changed/,
  );
});

test("binds local GitHub release assets to exact manifest IDs and bytes", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-release-mirror-"),
  );
  const assetDir = path.join(directory, "assets");
  await mkdir(assetDir);
  await writeFile(path.join(assetDir, "asset-a"), "macOS");
  await writeFile(path.join(assetDir, "asset-b"), "Windows");

  const release = {
    version: "1.4.0",
    status: "draft",
    assets: [
      {
        id: "asset-a",
        publicPlatform: "dmg-aarch64",
        size: 5,
        signature: null,
      },
      {
        id: "asset-b",
        publicPlatform: "nsis-x86_64",
        updatePlatform: "windows-x86_64-nsis",
        size: 7,
        signature: "signature",
      },
    ],
  };
  const output = path.join(directory, "manifest.json");
  await createManifest({
    release,
    output,
    version: "1.4.0",
    candidateSha,
    workflowRunId: "12345",
    cnVersion,
    cnAssetId,
    cnSha256,
    assetDir,
  });
  const manifest = JSON.parse(await readFile(output, "utf8"));
  const platformAssetIds = {
    "dmg-aarch64": "asset-a",
    "nsis-x86_64": "asset-b",
  };
  const verify = (assetIds = platformAssetIds) =>
    verifyLocalAssets({
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId,
      cnSha256,
      assetDir,
      platformAssetIds: assetIds,
    });

  await verify();

  await writeFile(path.join(assetDir, "asset-b"), "replace");
  await assert.rejects(verify(), /SHA-256 changed/);

  await writeFile(path.join(assetDir, "wrong-id"), "replace");
  await assert.rejects(
    verify(),
    /Local asset IDs do not match the provenance manifest/,
  );
});

test("rejects swapped public platform asset IDs with identical bytes", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-release-platform-map-"),
  );
  const assetDir = path.join(directory, "assets");
  await mkdir(assetDir);
  const contents = "identical payload";
  await writeFile(path.join(assetDir, "asset-a"), contents);
  await writeFile(path.join(assetDir, "asset-b"), contents);

  const release = {
    version: "1.4.0",
    status: "draft",
    assets: [
      {
        id: "asset-a",
        publicPlatform: "dmg-aarch64",
        size: Buffer.byteLength(contents),
        signature: null,
      },
      {
        id: "asset-b",
        publicPlatform: "nsis-x86_64",
        updatePlatform: "windows-x86_64-nsis",
        size: Buffer.byteLength(contents),
        signature: "signature",
      },
    ],
  };
  const output = path.join(directory, "manifest.json");
  await createManifest({
    release,
    output,
    version: "1.4.0",
    candidateSha,
    workflowRunId: "12345",
    cnVersion,
    cnAssetId,
    cnSha256,
    assetDir,
  });
  const manifest = JSON.parse(await readFile(output, "utf8"));

  await assert.rejects(
    verifyLocalAssets({
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId,
      cnSha256,
      assetDir,
      platformAssetIds: {
        "dmg-aarch64": "asset-b",
        "nsis-x86_64": "asset-a",
      },
    }),
    /Downloaded asset ID for dmg-aarch64 does not match the provenance manifest/,
  );
});
