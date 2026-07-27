import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_ASSET_BASE_URL = "https://cdn.crabnebula.app/asset";
const SOURCE_WORKFLOW = ".github/workflows/desktop_cd.yaml";
const CN_ACTION_REPOSITORY = "crabnebula-dev/cloud-release";

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

function normalizeCnActionRef(value) {
  invariant(
    typeof value === "string" &&
      new RegExp(`^${CN_ACTION_REPOSITORY}@[0-9a-f]{40}$`).test(value),
    "CrabNebula action ref must be pinned to a full commit SHA",
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
  cnActionRef,
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
        actionRef: normalizeCnActionRef(cnActionRef),
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
  cnActionRef,
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
    manifest.tools?.crabNebula?.actionRef === normalizeCnActionRef(cnActionRef),
    "CrabNebula action ref mismatch",
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

async function main() {
  const { command, args } = parseArgs(process.argv.slice(2));
  invariant(
    command === "create" || command === "verify",
    "Expected create or verify command",
  );

  const release = await readJson(args.release);
  const common = {
    release,
    version: args.version,
    candidateSha: args["candidate-sha"],
    workflowRunId: args["run-id"],
    cnVersion: args["cn-version"],
    cnActionRef: args["cn-action-ref"],
    assetDir: args["asset-dir"],
  };

  if (command === "create") {
    invariant(args.output, "Missing --output");
    await createManifest({ ...common, output: args.output });
    console.log(`Wrote release provenance to ${args.output}`);
    return;
  }

  invariant(args.manifest, "Missing --manifest");
  await verifyManifest({ ...common, manifest: await readJson(args.manifest) });
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
