import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createManifest,
  verifyLocalAssets,
  verifyManifest,
} from "./desktop-release-provenance.mjs";

const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const cnAssetId = "01KVDB8KPSKMQ5X3SJ0ANF6943";
const cnSha256 =
  "760b11d1ab9326dc78068ac8ef450685ea116e329903b14d94a5133641a54128";
const cnVersion = "cn 0.13.2";

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
  const verify = () =>
    verifyLocalAssets({
      manifest,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "12345",
      cnVersion,
      cnAssetId,
      cnSha256,
      assetDir,
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
