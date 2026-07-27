import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createManifest,
  verifyManifest,
} from "./desktop-release-provenance.mjs";

const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const cnActionRef =
  "crabnebula-dev/cloud-release@1a8803698ba41de6b23e42abc5dcc3721308233c";
const cnVersion = "cn 0.21.0";

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
    cnActionRef,
    assetDir,
  });
  const manifest = JSON.parse(await readFile(output, "utf8"));

  assert.deepEqual(manifest.tools, {
    crabNebula: { cliVersion: cnVersion, actionRef: cnActionRef },
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
    cnActionRef,
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
      cnActionRef,
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
      cnActionRef:
        "crabnebula-dev/cloud-release@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      assetDir,
    }),
    /action ref mismatch/,
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
      cnActionRef,
      assetDir,
    }),
    /size .* expected|SHA-256 changed/,
  );
});
