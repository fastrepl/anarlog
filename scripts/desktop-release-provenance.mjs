import { createHash, createPublicKey, verify } from "node:crypto";
import { createReadStream, readFileSync } from "node:fs";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_ASSET_BASE_URL = "https://cdn.crabnebula.app/asset";
const SOURCE_WORKFLOW = ".github/workflows/desktop_cd.yaml";
const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

export const gpuiArtifactSpecs = {
  "anarlog-gpui-stable-macos-silicon": {
    group: "macos",
    files: [
      {
        suffix: ".dmg",
        publicPlatform: "dmg-aarch64",
        updatePlatform: null,
        signed: false,
        hasSha256Sidecar: false,
      },
      {
        suffix: ".app.tar.gz",
        publicPlatform: null,
        updatePlatform: "darwin-aarch64",
        signed: true,
        hasSha256Sidecar: false,
      },
    ],
  },
  "anarlog-gpui-stable-macos-intel": {
    group: "macos",
    files: [
      {
        suffix: ".dmg",
        publicPlatform: "dmg-x86_64",
        updatePlatform: null,
        signed: false,
        hasSha256Sidecar: false,
      },
      {
        suffix: ".app.tar.gz",
        publicPlatform: null,
        updatePlatform: "darwin-x86_64",
        signed: true,
        hasSha256Sidecar: false,
      },
    ],
  },
  "anarlog-gpui-stable-linux-x64": {
    group: "linux",
    files: [
      {
        suffix: ".AppImage",
        publicPlatform: "appimage-x86_64",
        updatePlatform: "linux-x86_64-appimage",
        signed: true,
        hasSha256Sidecar: true,
      },
      {
        suffix: ".deb",
        publicPlatform: "debian-x86_64",
        updatePlatform: "linux-x86_64-deb",
        signed: true,
        hasSha256Sidecar: true,
      },
    ],
  },
  "anarlog-gpui-stable-linux-arm64": {
    group: "linux",
    files: [
      {
        suffix: ".AppImage",
        publicPlatform: "appimage-aarch64",
        updatePlatform: "linux-aarch64-appimage",
        signed: true,
        hasSha256Sidecar: true,
      },
      {
        suffix: ".deb",
        publicPlatform: "debian-aarch64",
        updatePlatform: "linux-aarch64-deb",
        signed: true,
        hasSha256Sidecar: true,
      },
    ],
  },
  "anarlog-gpui-stable-windows-x64": {
    group: "windows",
    files: [
      {
        suffix: "-setup.exe",
        publicPlatform: "nsis-x86_64",
        updatePlatform: "windows-x86_64-nsis",
        signed: true,
        hasSha256Sidecar: true,
      },
    ],
  },
};

// The authored plan is the single source for supported release platforms;
// workflow matrices are validated against it in the provenance tests.
export const releasePlatformPlan = JSON.parse(
  readFileSync(
    new URL("./desktop-release-platforms.json", import.meta.url),
    "utf8",
  ),
);

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

function plannedPlatforms(kind, { includeLinux, includeWindows }) {
  return [
    ...releasePlatformPlan.macos[kind],
    ...(includeLinux ? releasePlatformPlan.linux[kind] : []),
    ...(includeWindows ? releasePlatformPlan.windows[kind] : []),
  ].sort();
}

export function verifyDesktopPlatformSets(
  release,
  { includeLinux = true, includeWindows = true } = {},
) {
  const assets = normalizeAssets(release);
  const selection = { includeLinux, includeWindows };
  const expectedPublicPlatforms = plannedPlatforms(
    "publicPlatforms",
    selection,
  );
  const expectedUpdatePlatforms = plannedPlatforms(
    "updatePlatforms",
    selection,
  );
  invariant(
    assets.every(
      (asset) => asset.publicPlatform !== null || asset.updatePlatform !== null,
    ),
    "Every release asset must map to a public or update platform",
  );
  invariant(
    assets.every(
      (asset) =>
        asset.updatePlatform === null ||
        (typeof asset.signature === "string" && asset.signature.length > 0),
    ),
    "Every updater asset must carry a signature",
  );

  const publicPlatforms = assets
    .map((asset) => asset.publicPlatform)
    .filter((platform) => platform !== null)
    .sort();
  invariant(
    JSON.stringify(publicPlatforms) === JSON.stringify(expectedPublicPlatforms),
    `Release public platforms do not match the release plan (expected [${expectedPublicPlatforms}], got [${publicPlatforms}])`,
  );

  const updatePlatforms = assets
    .map((asset) => asset.updatePlatform)
    .filter((platform) => platform !== null)
    .sort();
  invariant(
    JSON.stringify(updatePlatforms) === JSON.stringify(expectedUpdatePlatforms),
    `Release update platforms do not match the release plan (expected [${expectedUpdatePlatforms}], got [${updatePlatforms}])`,
  );
}

// Validates workflow files against the authored release plan instead of
// letting each workflow repeat its own expected platform sets. The publish
// workflow must download exactly the planned public platforms, and the CD
// workflow must build every planned target triple.
export function verifyWorkflowPlatformCoverage({
  publishWorkflow,
  cdWorkflow,
  plan = releasePlatformPlan,
}) {
  const plannedPublic = Object.values(plan)
    .filter((group) => typeof group === "object" && group.publicPlatforms)
    .flatMap((group) => group.publicPlatforms)
    .sort();
  // A platform may be downloaded by more than one job (e.g. the Windows
  // signature check), so compare unique platform names.
  const downloadPlatforms = [
    ...new Set(
      [
        ...publishWorkflow.matchAll(/^\s*platform:\s*([A-Za-z0-9._-]+)\s*$/gm),
      ].map((match) => match[1]),
    ),
  ].sort();
  invariant(
    JSON.stringify(downloadPlatforms) === JSON.stringify(plannedPublic),
    `Publish workflow downloads [${downloadPlatforms}] do not match the release plan [${plannedPublic}]`,
  );

  const plannedTargets = Object.values(plan)
    .filter((group) => typeof group === "object" && group.buildTargets)
    .flatMap((group) => group.buildTargets);
  for (const target of plannedTargets) {
    invariant(
      cdWorkflow.includes(target),
      `Release workflow does not build the planned target ${target}`,
    );
  }
}

async function digestStream(stream, algorithm) {
  const hash = createHash(algorithm);
  let size = 0;

  for await (const chunk of stream) {
    hash.update(chunk);
    size += chunk.length;
  }

  return { digest: hash.digest(), size };
}

async function hashStream(stream) {
  const { digest, size } = await digestStream(stream, "sha256");
  return { sha256: digest.toString("hex"), size };
}

function decodeStrictBase64(value, label) {
  invariant(typeof value === "string", `${label} must be a string`);
  const trimmed = value.trim();
  invariant(
    trimmed.length > 0 &&
      trimmed.length % 4 === 0 &&
      /^[A-Za-z0-9+/]+={0,2}$/.test(trimmed),
    `${label} must be valid base64`,
  );
  return Buffer.from(trimmed, "base64");
}

function normalizeBasename(value, label) {
  invariant(
    typeof value === "string" &&
      value.length > 0 &&
      value !== "." &&
      value !== ".." &&
      !value.includes("/") &&
      !value.includes("\\") &&
      path.basename(value) === value,
    `${label} must be a safe file basename`,
  );
  return value;
}

export async function verifyMinisignSignature({
  filePath,
  signatureText,
  pubkeyBase64,
}) {
  const pubkeyDecoded = decodeStrictBase64(
    pubkeyBase64,
    "Updater public key",
  ).toString("utf8");
  const pubkeyLines = pubkeyDecoded.replace(/\r?\n$/, "").split(/\r?\n/);
  invariant(
    pubkeyLines.length === 2,
    "Minisign public key must contain two lines",
  );
  const pubkeyRaw = decodeStrictBase64(
    pubkeyLines[1],
    "Minisign public key body",
  );
  invariant(pubkeyRaw.length === 42, "Minisign public key must be 42 bytes");
  const pkAlg = pubkeyRaw.subarray(0, 2).toString("ascii");
  invariant(
    pkAlg === "Ed" || pkAlg === "ED",
    `Unsupported minisign public key algorithm ${pkAlg}`,
  );
  const pkKeyId = pubkeyRaw.subarray(2, 10);
  const publicKey = createPublicKey({
    key: Buffer.concat([ED25519_SPKI_PREFIX, pubkeyRaw.subarray(10, 42)]),
    format: "der",
    type: "spki",
  });

  const sigDecoded = decodeStrictBase64(
    signatureText,
    "Updater signature",
  ).toString("utf8");
  const sigLines = sigDecoded.replace(/\r?\n$/, "").split(/\r?\n/);
  invariant(sigLines.length >= 4, "Minisign signature must contain four lines");
  const sigBody = decodeStrictBase64(sigLines[1], "Minisign signature body");
  invariant(sigBody.length === 74, "Minisign signature body must be 74 bytes");
  const sigAlg = sigBody.subarray(0, 2).toString("ascii");
  invariant(
    sigAlg === "Ed" || sigAlg === "ED",
    `Unsupported minisign signature algorithm ${sigAlg}`,
  );
  const isPrehashed = sigAlg === "ED";
  const sigKeyId = sigBody.subarray(2, 10);
  invariant(
    sigKeyId.equals(pkKeyId),
    "Updater signature key ID does not match public key",
  );
  const sigBytes = sigBody.subarray(10, 74);

  const trustedPrefix = "trusted comment: ";
  invariant(
    sigLines[2].startsWith(trustedPrefix),
    "Minisign signature is missing trusted comment",
  );
  const trustedComment = sigLines[2].slice(trustedPrefix.length);
  const globalSigBytes = decodeStrictBase64(
    sigLines[3],
    "Minisign global signature",
  );
  invariant(
    globalSigBytes.length === 64,
    "Minisign global signature must be 64 bytes",
  );

  const payload = isPrehashed
    ? (await digestStream(createReadStream(filePath), "blake2b512")).digest
    : await readFile(filePath);
  invariant(
    verify(null, payload, publicKey, sigBytes),
    "Updater signature verification failed",
  );
  const globalPayload = Buffer.concat([
    sigBytes,
    Buffer.from(trustedComment, "utf8"),
  ]);
  invariant(
    verify(null, globalPayload, publicKey, globalSigBytes),
    "Updater global signature verification failed",
  );
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

async function collectArtifactFiles(artifactDir) {
  const topEntries = await readdir(artifactDir, { withFileTypes: true });
  const diskFiles = new Map();
  const byArtifact = new Map();

  for (const top of topEntries) {
    invariant(
      !top.isSymbolicLink() && top.isDirectory(),
      `Artifact directory must contain only artifact directories, found ${top.name}`,
    );
    const artifactName = top.name;
    const artifactRoot = path.join(artifactDir, artifactName);
    const fileList = [];

    async function walk(directory) {
      const entries = await readdir(directory, { withFileTypes: true });
      for (const entry of entries) {
        const entryPath = path.join(directory, entry.name);
        invariant(
          !entry.isSymbolicLink(),
          `Unexpected symlink in artifact ${artifactName}: ${entryPath}`,
        );
        if (entry.isDirectory()) {
          await walk(entryPath);
        } else if (entry.isFile()) {
          const key = `${artifactName}/${entry.name}`;
          invariant(
            !diskFiles.has(key),
            `Duplicate file basename in artifact ${artifactName}: ${entry.name}`,
          );
          const record = {
            artifact: artifactName,
            name: entry.name,
            filePath: entryPath,
          };
          diskFiles.set(key, record);
          fileList.push(record);
        } else {
          throw new Error(
            `Unsupported entry type in artifact ${artifactName}: ${entry.name}`,
          );
        }
      }
    }

    await walk(artifactRoot);
    byArtifact.set(artifactName, fileList);
  }

  return { diskFiles, byArtifact };
}

function selectedGpuiArtifactNames({
  includeLinux,
  includeWindows,
  artifactFilter,
}) {
  if (artifactFilter) {
    invariant(
      Object.hasOwn(gpuiArtifactSpecs, artifactFilter),
      `Unknown GPUI artifact ${artifactFilter}`,
    );
    return [artifactFilter];
  }
  return Object.entries(gpuiArtifactSpecs)
    .filter(
      ([, spec]) =>
        spec.group === "macos" ||
        (spec.group === "linux" && includeLinux) ||
        (spec.group === "windows" && includeWindows),
    )
    .map(([name]) => name);
}

export async function verifyGpuiArtifacts({
  manifest,
  artifactDir,
  version,
  candidateSha,
  workflowRunId,
  tauriConfigPath,
  includeLinux = true,
  includeWindows = true,
  artifactFilter = null,
  planOutput = null,
}) {
  invariant(artifactDir, "Missing GPUI artifact directory");
  invariant(tauriConfigPath, "Missing Tauri config path");
  invariant(
    manifest?.schemaVersion === 1,
    "Unsupported GPUI provenance manifest schema",
  );
  invariant(manifest.app === "gpui", "Provenance manifest app must be gpui");
  invariant(
    manifest.sourceWorkflow === SOURCE_WORKFLOW,
    "Unexpected GPUI source workflow",
  );
  invariant(
    manifest.workflowRunId === normalizeRunId(workflowRunId),
    "GPUI workflow run ID mismatch",
  );
  invariant(
    String(manifest.runAttempt) === "1",
    "GPUI provenance must come from an unrerun workflow attempt",
  );
  const normalizedSha = normalizeSha(candidateSha);
  invariant(
    manifest.candidateSha === normalizedSha,
    "GPUI candidate SHA mismatch",
  );
  invariant(manifest.version === version, "GPUI manifest version mismatch");
  invariant(
    manifest.channel === "stable",
    "GPUI manifest channel must be stable",
  );
  invariant(
    Array.isArray(manifest.assets) && manifest.assets.length > 0,
    "GPUI provenance manifest has no assets",
  );

  const normalizedEntries = new Map();
  for (const entry of manifest.assets) {
    invariant(
      typeof entry?.artifact === "string" &&
        Object.hasOwn(gpuiArtifactSpecs, entry.artifact),
      `Unexpected GPUI artifact ${entry?.artifact}`,
    );
    const name = normalizeBasename(entry.name, "GPUI manifest asset name");
    const size = Number(entry.size);
    invariant(
      Number.isSafeInteger(size) && size > 0,
      `GPUI asset ${entry.artifact}/${name} has invalid size`,
    );
    const sha256 = normalizeSha256(
      entry.sha256,
      `GPUI asset ${entry.artifact}/${name} SHA-256`,
    );
    const key = `${entry.artifact}/${name}`;
    invariant(
      !normalizedEntries.has(key),
      `Duplicate GPUI manifest entry ${key}`,
    );
    normalizedEntries.set(key, {
      artifact: entry.artifact,
      name,
      size,
      sha256,
    });
  }

  const expectedEntries = new Map();
  if (artifactFilter) {
    invariant(
      Object.hasOwn(gpuiArtifactSpecs, artifactFilter),
      `Unknown GPUI artifact ${artifactFilter}`,
    );
    for (const [key, entry] of normalizedEntries) {
      if (entry.artifact === artifactFilter) {
        expectedEntries.set(key, entry);
      }
    }
    invariant(
      expectedEntries.size > 0,
      `GPUI provenance manifest has no entries for ${artifactFilter}`,
    );
  } else {
    for (const [key, entry] of normalizedEntries) {
      expectedEntries.set(key, entry);
    }
  }

  const { diskFiles, byArtifact } = await collectArtifactFiles(artifactDir);
  const expectedKeys = [...expectedEntries.keys()].sort();
  const diskKeys = [...diskFiles.keys()].sort();
  invariant(
    JSON.stringify(diskKeys) === JSON.stringify(expectedKeys),
    `Downloaded GPUI artifacts do not match provenance manifest (expected [${expectedKeys}], got [${diskKeys}])`,
  );

  for (const key of expectedKeys) {
    const expected = expectedEntries.get(key);
    const diskEntry = diskFiles.get(key);
    const actual = await hashStream(createReadStream(diskEntry.filePath));
    invariant(
      actual.size === expected.size,
      `GPUI asset ${key} has size ${actual.size}, expected ${expected.size}`,
    );
    invariant(
      actual.sha256 === expected.sha256,
      `GPUI asset ${key} SHA-256 mismatch`,
    );
    diskEntry.size = actual.size;
    diskEntry.sha256 = actual.sha256;
  }

  const tauriConfig = await readJson(tauriConfigPath);
  const pubkeyBase64 = tauriConfig?.plugins?.updater?.pubkey;
  invariant(
    typeof pubkeyBase64 === "string" && pubkeyBase64.trim().length > 0,
    "Tauri config is missing plugins.updater.pubkey",
  );

  const selectedArtifacts = selectedGpuiArtifactNames({
    includeLinux,
    includeWindows,
    artifactFilter,
  });
  const uploads = [];

  for (const artifactName of selectedArtifacts) {
    const filesInArtifact = byArtifact.get(artifactName);
    invariant(
      Array.isArray(filesInArtifact),
      `Missing downloaded GPUI artifact directory ${artifactName}`,
    );
    const byName = new Map(filesInArtifact.map((file) => [file.name, file]));
    const accounted = new Set();

    for (const spec of gpuiArtifactSpecs[artifactName].files) {
      const matches = filesInArtifact.filter((file) =>
        file.name.endsWith(spec.suffix),
      );
      invariant(
        matches.length === 1,
        `Expected one ${spec.suffix} file in ${artifactName}, found ${matches.length}`,
      );
      const payload = matches[0];
      accounted.add(payload.name);

      const sigName = `${payload.name}.sig`;
      let sigEntry = null;
      let signatureText = null;
      if (spec.signed) {
        sigEntry = byName.get(sigName) ?? null;
        invariant(
          sigEntry,
          `Missing updater signature ${artifactName}/${sigName}`,
        );
        accounted.add(sigName);
        signatureText = (await readFile(sigEntry.filePath, "utf8")).trim();
        await verifyMinisignSignature({
          filePath: payload.filePath,
          signatureText,
          pubkeyBase64,
        });
      } else {
        invariant(
          !byName.has(sigName),
          `Unexpected updater signature ${artifactName}/${sigName}`,
        );
      }

      if (spec.hasSha256Sidecar) {
        const sidecarName = `${payload.name}.sha256`;
        const sidecarEntry = byName.get(sidecarName);
        invariant(
          sidecarEntry,
          `Missing SHA-256 sidecar ${artifactName}/${sidecarName}`,
        );
        accounted.add(sidecarName);
        const sidecarText = (
          await readFile(sidecarEntry.filePath, "utf8")
        ).trim();
        invariant(
          sidecarText === `${payload.sha256}  ${payload.name}`,
          `Checksum sidecar ${artifactName}/${sidecarName} does not match verified SHA-256`,
        );
      }

      uploads.push({
        artifact: artifactName,
        name: payload.name,
        file: path.resolve(payload.filePath),
        size: payload.size,
        sha256: payload.sha256,
        publicPlatform: spec.publicPlatform,
        updatePlatform: spec.updatePlatform,
        signatureFile: sigEntry ? path.resolve(sigEntry.filePath) : null,
        signature: signatureText,
      });
    }

    invariant(
      accounted.size === filesInArtifact.length,
      `Artifact ${artifactName} contains unexpected extra files`,
    );
  }

  if (!artifactFilter) {
    const plannedPublic = plannedPlatforms("publicPlatforms", {
      includeLinux,
      includeWindows,
    });
    const actualPublic = uploads
      .map((upload) => upload.publicPlatform)
      .filter((platform) => platform !== null)
      .sort();
    invariant(
      JSON.stringify(actualPublic) === JSON.stringify(plannedPublic),
      `GPUI upload public platforms [${actualPublic}] do not match release plan [${plannedPublic}]`,
    );

    const plannedUpdate = plannedPlatforms("updatePlatforms", {
      includeLinux,
      includeWindows,
    });
    const actualUpdate = uploads
      .map((upload) => upload.updatePlatform)
      .filter((platform) => platform !== null)
      .sort();
    invariant(
      JSON.stringify(actualUpdate) === JSON.stringify(plannedUpdate),
      `GPUI upload update platforms [${actualUpdate}] do not match release plan [${plannedUpdate}]`,
    );
  }

  const plan = {
    schemaVersion: 1,
    app: "gpui",
    version,
    candidateSha: normalizedSha,
    workflowRunId: normalizeRunId(workflowRunId),
    includeLinux,
    includeWindows,
    uploads,
  };

  if (planOutput) {
    await writeFile(planOutput, `${JSON.stringify(plan, null, 2)}\n`);
  }

  return plan;
}

export async function verifyGpuiRelease({
  release,
  plan,
  version,
  includeLinux = true,
  includeWindows = true,
  assetDir = undefined,
}) {
  invariant(
    plan?.schemaVersion === 1 && plan.app === "gpui",
    "Invalid GPUI upload plan",
  );
  invariant(plan.version === version, "GPUI upload plan version mismatch");
  invariant(
    plan.includeLinux === includeLinux &&
      plan.includeWindows === includeWindows,
    "GPUI upload plan platform selection mismatch",
  );
  invariant(
    Array.isArray(plan.uploads) && plan.uploads.length > 0,
    "GPUI upload plan has no uploads",
  );
  invariant(
    release?.version === version,
    `Release version is ${release?.version}, expected ${version}`,
  );

  verifyDesktopPlatformSets(release, { includeLinux, includeWindows });

  const releaseAssets = normalizeAssets(release);
  invariant(
    releaseAssets.length === plan.uploads.length,
    `Release asset count (${releaseAssets.length}) does not match upload plan (${plan.uploads.length})`,
  );

  const uploadsByPlatform = new Map();
  for (const upload of plan.uploads) {
    const key = `${upload.publicPlatform ?? ""}::${upload.updatePlatform ?? ""}`;
    invariant(
      !uploadsByPlatform.has(key),
      `Duplicate upload platform pair ${key}`,
    );
    uploadsByPlatform.set(key, upload);
  }

  const matchedKeys = new Set();
  for (const asset of releaseAssets) {
    const key = `${asset.publicPlatform ?? ""}::${asset.updatePlatform ?? ""}`;
    const planned = uploadsByPlatform.get(key);
    invariant(planned, `Unexpected CrabNebula asset platform pair ${key}`);
    invariant(
      !matchedKeys.has(key),
      `Duplicate CrabNebula asset platform pair ${key}`,
    );
    matchedKeys.add(key);

    invariant(
      asset.size === planned.size,
      `CrabNebula asset ${asset.id} (${planned.name}) has size ${asset.size}, expected ${planned.size}`,
    );
    const actualSignature = asset.signature ? asset.signature.trim() : null;
    const expectedSignature = planned.signature
      ? planned.signature.trim()
      : null;
    invariant(
      actualSignature === expectedSignature,
      `CrabNebula asset ${asset.id} (${planned.name}) signature mismatch`,
    );

    const actualSha256 = await hashAsset(asset, assetDir);
    invariant(
      actualSha256 === planned.sha256,
      `CrabNebula asset ${asset.id} (${planned.name}) SHA-256 mismatch`,
    );
  }
}

async function main() {
  const { command, args } = parseArgs(process.argv.slice(2));
  invariant(
    command === "create" ||
      command === "verify" ||
      command === "verify-assets" ||
      command === "verify-platforms" ||
      command === "verify-gpui" ||
      command === "verify-gpui-release",
    "Expected create, verify, verify-assets, verify-platforms, verify-gpui, or verify-gpui-release command",
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

  if (command === "verify-gpui") {
    invariant(args.manifest, "Missing --manifest");
    const plan = await verifyGpuiArtifacts({
      manifest: await readJson(args.manifest),
      artifactDir: args["artifact-dir"],
      version: args.version,
      candidateSha: args["candidate-sha"],
      workflowRunId: args["run-id"],
      tauriConfigPath: args["tauri-config"],
      includeLinux: normalizeBoolean(
        args["include-linux"] ?? "true",
        "include-linux",
      ),
      includeWindows: normalizeBoolean(
        args["include-windows"] ?? "true",
        "include-windows",
      ),
      artifactFilter: args.artifact ?? null,
      planOutput: args["plan-output"] ?? null,
    });
    console.log(
      `Verified ${plan.uploads.length} GPUI release assets against provenance`,
    );
    return;
  }

  if (command === "verify-gpui-release") {
    invariant(args.release, "Missing --release");
    invariant(args.plan, "Missing --plan");
    await verifyGpuiRelease({
      release: await readJson(args.release),
      plan: await readJson(args.plan),
      version: args.version,
      includeLinux: normalizeBoolean(
        args["include-linux"] ?? "true",
        "include-linux",
      ),
      includeWindows: normalizeBoolean(
        args["include-windows"] ?? "true",
        "include-windows",
      ),
      assetDir: args["asset-dir"],
    });
    console.log("CrabNebula GPUI release assets match verified provenance");
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
