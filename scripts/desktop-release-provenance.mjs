import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_ASSET_BASE_URL = "https://cdn.crabnebula.app/asset";
const SOURCE_WORKFLOW = ".github/workflows/desktop_cd.yaml";
const MACOS_PUBLIC_PLATFORMS = ["dmg-aarch64", "dmg-x86_64"];
const LINUX_PUBLIC_PLATFORMS = [
  "appimage-aarch64",
  "appimage-x86_64",
  "debian-aarch64",
  "debian-x86_64",
];
const WINDOWS_PUBLIC_PLATFORMS = ["nsis-x86_64"];
const MACOS_UPDATE_PLATFORMS = ["darwin-aarch64", "darwin-x86_64"];
const LINUX_UPDATE_PLATFORMS = [
  "linux-aarch64-appimage",
  "linux-aarch64-deb",
  "linux-x86_64-appimage",
  "linux-x86_64-deb",
];
const WINDOWS_UPDATE_PLATFORMS = ["windows-x86_64-nsis"];

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const args = {};

  for (let index = 0; index < rest.length; index += 2) {
    const key = rest[index];
    const value = rest[index + 1];
    invariant(key?.startsWith("--") && value, `Invalid argument: ${key ?? ""}`);
    args[key.slice(2)] = value;
  }

  return { command, args };
}

function normalizeSha(value) {
  invariant(
    typeof value === "string" && /^[0-9a-fA-F]{40}$/.test(value),
    "Candidate SHA must contain 40 hexadecimal characters",
  );
  return value.toLowerCase();
}

function normalizeRunId(value) {
  const runId = String(value);
  invariant(
    /^[1-9][0-9]*$/.test(runId),
    "Workflow run ID must be a positive integer",
  );
  return runId;
}

function normalizeBoolean(value, label) {
  invariant(
    value === "true" || value === "false",
    `${label} must be true or false`,
  );
  return value === "true";
}

function normalizeCnVersion(value) {
  invariant(
    typeof value === "string" &&
      value.trim().length > 0 &&
      value.trim().length <= 200 &&
      !/[\r\n]/.test(value.trim()),
    "CrabNebula CLI version must be one non-empty line",
  );
  return value.trim();
}

function normalizeCnAssetId(value) {
  invariant(
    typeof value === "string" && /^[A-Za-z0-9_-]+$/.test(value),
    "CrabNebula CLI asset ID is invalid",
  );
  return value;
}

function normalizeSha256(value, label) {
  invariant(
    typeof value === "string" && /^[0-9a-f]{64}$/.test(value),
    `${label} must be a lowercase SHA-256`,
  );
  return value;
}

function normalizeAsset(asset) {
  invariant(
    typeof asset?.id === "string" && /^[A-Za-z0-9_-]+$/.test(asset.id),
    "Every CrabNebula asset must have a safe ID",
  );

  const size = Number(asset.size);
  invariant(
    Number.isSafeInteger(size) && size > 0,
    `Asset ${asset.id} has an invalid size`,
  );

  const publicPlatform = asset.publicPlatform ?? null;
  const updatePlatform = asset.updatePlatform ?? null;
  const signature = asset.signature ?? null;
  invariant(
    publicPlatform === null || typeof publicPlatform === "string",
    `Asset ${asset.id} has an invalid public platform`,
  );
  invariant(
    updatePlatform === null || typeof updatePlatform === "string",
    `Asset ${asset.id} has an invalid update platform`,
  );
  invariant(
    signature === null || typeof signature === "string",
    `Asset ${asset.id} has an invalid signature`,
  );

  return {
    id: asset.id,
    publicPlatform,
    updatePlatform,
    size,
    signature,
  };
}

function normalizeAssets(release) {
  invariant(
    Array.isArray(release?.assets) && release.assets.length > 0,
    "Release has no assets",
  );

  const assets = release.assets
    .map(normalizeAsset)
    .sort((left, right) => left.id.localeCompare(right.id));
  invariant(
    new Set(assets.map((asset) => asset.id)).size === assets.length,
    "Release contains duplicate asset IDs",
  );
  return assets;
}

export function verifyDesktopPlatformSets(
  release,
  { includeLinux = true, includeWindows = true } = {},
) {
  const assets = normalizeAssets(release);
  const expectedPublicPlatforms = [
    ...MACOS_PUBLIC_PLATFORMS,
    ...(includeLinux ? LINUX_PUBLIC_PLATFORMS : []),
    ...(includeWindows ? WINDOWS_PUBLIC_PLATFORMS : []),
  ].sort();
  const expectedUpdatePlatforms = [
    ...MACOS_UPDATE_PLATFORMS,
    ...(includeLinux ? LINUX_UPDATE_PLATFORMS : []),
    ...(includeWindows ? WINDOWS_UPDATE_PLATFORMS : []),
  ].sort();
  invariant(
    assets.length === expectedPublicPlatforms.length,
    `Release must contain exactly ${expectedPublicPlatforms.length} selected desktop assets`,
  );

  const publicPlatforms = assets
    .map((asset) => asset.publicPlatform)
    .filter((platform) => platform !== null)
    .sort();
  invariant(
    JSON.stringify(publicPlatforms) === JSON.stringify(expectedPublicPlatforms),
    "Release public platforms do not match the selected desktop platforms",
  );

  const updatePlatforms = assets
    .map((asset) => asset.updatePlatform)
    .filter((platform) => platform !== null)
    .sort();
  invariant(
    JSON.stringify(updatePlatforms) === JSON.stringify(expectedUpdatePlatforms),
    "Release update platforms do not match the selected desktop platforms",
  );
}

async function hashStream(stream) {
  const hash = createHash("sha256");
  let size = 0;

  for await (const chunk of stream) {
    hash.update(chunk);
    size += chunk.length;
  }

  return { sha256: hash.digest("hex"), size };
}

async function hashAsset(asset, assetDir) {
  const result = assetDir
    ? await hashStream(createReadStream(path.join(assetDir, asset.id)))
    : await hashRemoteAsset(asset.id);

  invariant(
    result.size === asset.size,
    `Asset ${asset.id} has size ${result.size}, expected ${asset.size}`,
  );
  return result.sha256;
}

async function hashRemoteAsset(assetId) {
  const baseUrl = process.env.CN_ASSET_BASE_URL ?? DEFAULT_ASSET_BASE_URL;
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}/${assetId}`);
  invariant(
    response.ok && response.body,
    `Could not download asset ${assetId}: ${response.status}`,
  );
  return hashStream(response.body);
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

export async function createManifest({
  release,
  output,
  version,
  candidateSha,
  workflowRunId,
  cnVersion,
  cnAssetId,
  cnSha256,
  assetDir,
}) {
  invariant(
    release?.version === version,
    `Release version is ${release?.version}, expected ${version}`,
  );
  invariant(
    String(release?.status ?? "").toLowerCase() === "draft",
    "A provenance manifest can only be created from a draft release",
  );

  const assets = normalizeAssets(release);
  const hashedAssets = [];
  for (const asset of assets) {
    hashedAssets.push({
      ...asset,
      sha256: await hashAsset(asset, assetDir),
    });
  }

  const manifest = {
    schemaVersion: 1,
    sourceWorkflow: SOURCE_WORKFLOW,
    workflowRunId: normalizeRunId(workflowRunId),
    candidateSha: normalizeSha(candidateSha),
    version,
    channel: "stable",
    publish: false,
    tools: {
      crabNebula: {
        cliVersion: normalizeCnVersion(cnVersion),
        cliAssetId: normalizeCnAssetId(cnAssetId),
        cliSha256: normalizeSha256(cnSha256, "CrabNebula CLI SHA-256"),
      },
    },
    assets: hashedAssets,
  };

  await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

export async function verifyManifest({
  release,
  manifest,
  version,
  candidateSha,
  workflowRunId,
  cnVersion,
  cnAssetId,
  cnSha256,
  assetDir,
}) {
  invariant(
    manifest?.schemaVersion === 1,
    "Unsupported provenance manifest schema",
  );
  invariant(
    manifest.sourceWorkflow === SOURCE_WORKFLOW,
    "Unexpected source workflow",
  );
  invariant(
    manifest.workflowRunId === normalizeRunId(workflowRunId),
    "Workflow run ID mismatch",
  );
  invariant(
    manifest.candidateSha === normalizeSha(candidateSha),
    "Candidate SHA mismatch",
  );
  invariant(manifest.version === version, "Manifest version mismatch");
  invariant(manifest.channel === "stable", "Manifest channel is not stable");
  invariant(
    manifest.publish === false,
    "Provenance must come from a publish=false run",
  );
  invariant(
    manifest.tools?.crabNebula?.cliVersion === normalizeCnVersion(cnVersion),
    "CrabNebula CLI version mismatch",
  );
  invariant(
    manifest.tools?.crabNebula?.cliAssetId === normalizeCnAssetId(cnAssetId),
    "CrabNebula CLI asset ID mismatch",
  );
  invariant(
    manifest.tools?.crabNebula?.cliSha256 ===
      normalizeSha256(cnSha256, "CrabNebula CLI SHA-256"),
    "CrabNebula CLI SHA-256 mismatch",
  );
  invariant(
    release?.version === version,
    `Release version is ${release?.version}, expected ${version}`,
  );

  const currentAssets = normalizeAssets(release);
  invariant(
    Array.isArray(manifest.assets) &&
      manifest.assets.length === currentAssets.length,
    "Release asset count does not match the provenance manifest",
  );

  for (let index = 0; index < currentAssets.length; index += 1) {
    const current = currentAssets[index];
    const recorded = manifest.assets[index];
    const { sha256, ...recordedMetadata } = recorded ?? {};
    invariant(
      JSON.stringify(recordedMetadata) === JSON.stringify(current),
      `Asset metadata changed at index ${index}`,
    );
    invariant(
      typeof sha256 === "string" && /^[0-9a-f]{64}$/.test(sha256),
      `Asset ${current.id} has an invalid recorded SHA-256`,
    );

    const currentSha256 = await hashAsset(current, assetDir);
    invariant(currentSha256 === sha256, `Asset ${current.id} SHA-256 changed`);
  }
}

export async function verifyLocalAssets({
  manifest,
  version,
  candidateSha,
  workflowRunId,
  cnVersion,
  cnAssetId,
  cnSha256,
  assetDir,
  platformAssetIds,
}) {
  invariant(assetDir, "Missing local asset directory");
  invariant(
    Array.isArray(manifest?.assets),
    "Provenance manifest has no asset list",
  );
  const entries = await readdir(assetDir, { withFileTypes: true });
  invariant(
    entries.every((entry) => entry.isFile()),
    "Local asset directory must contain only regular files",
  );

  const mirroredAssets = manifest.assets.filter(
    (asset) => typeof asset?.publicPlatform === "string",
  );
  invariant(mirroredAssets.length > 0, "Provenance has no public assets");
  invariant(
    platformAssetIds &&
      typeof platformAssetIds === "object" &&
      !Array.isArray(platformAssetIds),
    "Missing downloaded platform asset IDs",
  );
  const expectedPlatforms = mirroredAssets
    .map((asset) => asset.publicPlatform)
    .sort();
  invariant(
    new Set(expectedPlatforms).size === expectedPlatforms.length,
    "Provenance contains duplicate public platforms",
  );
  const downloadedPlatforms = Object.keys(platformAssetIds).sort();
  invariant(
    JSON.stringify(downloadedPlatforms) === JSON.stringify(expectedPlatforms),
    "Downloaded public platforms do not match the provenance manifest",
  );
  for (const asset of mirroredAssets) {
    const downloadedAssetId = normalizeCnAssetId(
      platformAssetIds[asset.publicPlatform],
    );
    invariant(
      downloadedAssetId === normalizeCnAssetId(asset.id),
      `Downloaded asset ID for ${asset.publicPlatform} does not match the provenance manifest`,
    );
  }

  const expectedAssetIds = mirroredAssets
    .map((asset) => normalizeCnAssetId(asset?.id))
    .sort();
  const actualAssetIds = entries.map((entry) => entry.name).sort();
  invariant(
    JSON.stringify(actualAssetIds) === JSON.stringify(expectedAssetIds),
    "Local asset IDs do not match the provenance manifest",
  );

  const release = {
    version,
    status: "draft",
    assets: mirroredAssets.map(({ sha256: _sha256, ...asset }) => asset),
  };
  await verifyManifest({
    release,
    manifest: { ...manifest, assets: mirroredAssets },
    version,
    candidateSha,
    workflowRunId,
    cnVersion,
    cnAssetId,
    cnSha256,
    assetDir,
  });
}

async function main() {
  const { command, args } = parseArgs(process.argv.slice(2));
  invariant(
    command === "create" ||
      command === "verify" ||
      command === "verify-assets" ||
      command === "verify-platforms",
    "Expected create, verify, verify-assets, or verify-platforms command",
  );

  const common = {
    version: args.version,
    candidateSha: args["candidate-sha"],
    workflowRunId: args["run-id"],
    cnVersion: args["cn-version"],
    cnAssetId: args["cn-asset-id"],
    cnSha256: args["cn-sha256"],
    assetDir: args["asset-dir"],
  };

  if (command === "create") {
    invariant(args.output, "Missing --output");
    await createManifest({
      ...common,
      release: await readJson(args.release),
      output: args.output,
    });
    console.log(`Wrote release provenance to ${args.output}`);
    return;
  }

  if (command === "verify-platforms") {
    invariant(args.release, "Missing --release");
    verifyDesktopPlatformSets(await readJson(args.release), {
      includeLinux: normalizeBoolean(
        args["include-linux"] ?? "true",
        "include-linux",
      ),
      includeWindows: normalizeBoolean(
        args["include-windows"] ?? "true",
        "include-windows",
      ),
    });
    console.log("Release desktop platforms match the selected release plan");
    return;
  }

  invariant(args.manifest, "Missing --manifest");
  const manifest = await readJson(args.manifest);
  if (command === "verify-assets") {
    let platformAssetIds;
    try {
      platformAssetIds = JSON.parse(args["platform-asset-ids"]);
    } catch {
      throw new Error("Downloaded platform asset IDs must be valid JSON");
    }
    await verifyLocalAssets({ ...common, manifest, platformAssetIds });
    console.log("Local release assets match provenance");
    return;
  }

  await verifyManifest({
    ...common,
    release: await readJson(args.release),
    manifest,
  });
  console.log("Release provenance verified");
}

const isMain = process.argv[1]
  ? import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
  : false;

if (isMain) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
