import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createManifest,
  gpuiArtifactSpecs,
  releasePlatformPlan,
  verifyDesktopPlatformSets,
  verifyGpuiArtifacts,
  verifyGpuiRelease,
  verifyLocalAssets,
  verifyManifest,
  verifyMinisignSignature,
  verifyWorkflowPlatformCoverage,
} from "./desktop-release-provenance.mjs";

const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const cnAssetId = "01KVDB8KPSKMQ5X3SJ0ANF6943";
const cnSha256 =
  "760b11d1ab9326dc78068ac8ef450685ea116e329903b14d94a5133641a54128";
const cnVersion = "cn 0.13.2";

test("a rejected Store package update cannot publish the previous submission", async (t) => {
  const workflow = await readFile(
    ".github/workflows/desktop_store_publish.yaml",
    "utf8",
  );
  const step = workflow.slice(
    workflow.indexOf("name: Submit Microsoft Store update"),
  );
  const block = step.match(/        run: \|\n((?:          .*\n|\n)+)/);
  assert.ok(block, "missing Store submission script");
  const root = await mkdtemp(path.join(os.tmpdir(), "store-submission-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const stub = path.join(root, "msstore");
  await writeFile(
    stub,
    `#!/bin/sh
case "$1 $2" in
  "submission update") touch update-rejected; exit 23 ;;
  "submission publish") touch published ;;
esac
exit 0
`,
  );
  await chmod(stub, 0o755);
  await writeFile(
    path.join(root, "microsoft-store-package.json"),
    '{"packages":[]}',
  );
  const script = path.join(root, "submit.ps1");
  await writeFile(script, block[1].replace(/^          /gm, ""));
  const result = spawnSync(
    "pwsh",
    ["-NoProfile", "-NonInteractive", "-File", script],
    {
      cwd: root,
      env: {
        ...process.env,
        PATH: `${root}${path.delimiter}${process.env.PATH}`,
        AZURE_AD_APPLICATION_CLIENT_ID: "fixture",
        AZURE_AD_APPLICATION_SECRET: "fixture",
        AZURE_AD_TENANT_ID: "fixture",
        MICROSOFT_STORE_PRODUCT_ID: "fixture",
        SELLER_ID: "fixture",
      },
      encoding: "utf8",
      timeout: 30_000,
    },
  );
  assert.ifError(result.error);
  assert.equal(result.signal, null);
  await readFile(path.join(root, "update-rejected"));
  assert.notEqual(result.status, 0, result.stdout + result.stderr);
  await assert.rejects(readFile(path.join(root, "published")), {
    code: "ENOENT",
  });
});

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

test("Store inspection cannot mutate submissions or retry rate limits", async (t) => {
  const workflow = await readFile(
    ".github/workflows/desktop_store_status.yaml",
    "utf8",
  );
  const block = workflow.match(/        run: \|\n((?:          .*\n|\n)+)/);
  assert.ok(block, "missing Store inspection script");
  const root = await mkdtemp(path.join(os.tmpdir(), "store-status-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const script = path.join(root, "inspect.ps1");
  await writeFile(
    script,
    `function Invoke-RestMethod {
  param($Method, $Uri, $Body, $TimeoutSec)
  if ($Method -ne 'Post' -or $Uri -ne 'https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/oauth2/v2.0/token') {
    Set-Content mutation 'unexpected request'
    throw 'Unexpected authentication request'
  }
  return @{ access_token = 'test-token' }
}
function Invoke-WebRequest {
  param($Method, $Uri, $Headers, $TimeoutSec, [switch]$SkipHttpErrorCheck)
  if ($Method -ne 'Get') {
    Set-Content mutation 'Store mutation attempted'
    throw 'Store mutation attempted'
  }
  if (Test-Path rate-limited) {
    Set-Content retried 'request after rate limit'
    throw 'Retried rate limit'
  }
  if ($env:TEST_RATE_LIMIT -eq 'true') {
    Set-Content rate-limited '429'
    return @{ StatusCode = 429; Headers = @{ 'Retry-After' = '600' }; Content = '{}' }
  }
  return @{ StatusCode = 200; Headers = @{}; Content = (@{ isSuccess = $true; responseData = @{ endpoint = $Uri } } | ConvertTo-Json) }
}
${block[1].replace(/^          /gm, "")}`,
  );
  for (const rateLimit of [false, true]) {
    const result = spawnSync("pwsh", ["-NoProfile", "-File", script], {
      cwd: root,
      encoding: "utf8",
      timeout: 15_000,
      env: {
        ...process.env,
        SUBMISSION_ID: "123",
        MICROSOFT_STORE_PRODUCT_ID: "22222222-2222-2222-2222-222222222222",
        AZURE_AD_TENANT_ID: "11111111-1111-1111-1111-111111111111",
        TEST_RATE_LIMIT: String(rateLimit),
      },
    });
    assert.ifError(result.error);
    await assert.rejects(readFile(path.join(root, "mutation")), {
      code: "ENOENT",
    });
    await assert.rejects(readFile(path.join(root, "retried")), {
      code: "ENOENT",
    });
    if (rateLimit) {
      assert.notEqual(result.status, 0, result.stdout);
      assert.equal(
        await readFile(path.join(root, "rate-limited"), "utf8"),
        "429\n",
      );
    } else {
      assert.equal(result.status, 0, result.stderr);
      const endpoints = [
        ...result.stdout.matchAll(/"endpoint": "([^"]+)"/g),
      ].map((match) => match[1]);
      const base =
        "https://api.store.microsoft.com/submission/v1/product/22222222-2222-2222-2222-222222222222";
      assert.deepEqual(
        endpoints.sort(),
        [
          `${base}/packages`,
          `${base}/status`,
          `${base}/submission/123/status`,
        ].sort(),
      );
    }
  }
});

function createMinisignSigner() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const spki = publicKey.export({ format: "der", type: "spki" });
  const rawPublic = spki.subarray(-32);
  const keyId = Buffer.from("0102030405060708", "hex");
  const pkPayload = Buffer.concat([
    Buffer.from("Ed", "ascii"),
    keyId,
    rawPublic,
  ]);
  const pubkeyBase64 = Buffer.from(
    `untrusted comment: minisign public key\n${pkPayload.toString("base64")}\n`,
    "utf8",
  ).toString("base64");

  const signFile = (content, fileName) => {
    const digest = createHash("blake2b512").update(content).digest();
    const signature = sign(null, digest, privateKey);
    const sigBody = Buffer.concat([
      Buffer.from("ED", "ascii"),
      keyId,
      signature,
    ]);
    const trustedComment = `timestamp:1700000000\tfile:${fileName}\thashed`;
    const globalSignature = sign(
      null,
      Buffer.concat([signature, Buffer.from(trustedComment, "utf8")]),
      privateKey,
    );
    const decoded = [
      "untrusted comment: signature from minisign secret key",
      sigBody.toString("base64"),
      `trusted comment: ${trustedComment}`,
      globalSignature.toString("base64"),
    ].join("\n");
    return Buffer.from(`${decoded}\n`, "utf8").toString("base64");
  };

  return { pubkeyBase64, signFile };
}

async function createGpuiFixture(t, { version = "1.4.0" } = {}) {
  const root = await mkdtemp(path.join(os.tmpdir(), "anarlog-gpui-prov-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const signer = createMinisignSigner();
  const tauriConfigPath = path.join(root, "tauri.conf.json");
  await writeFile(
    tauriConfigPath,
    JSON.stringify({ plugins: { updater: { pubkey: signer.pubkeyBase64 } } }),
  );

  const artifactDir = path.join(root, "artifacts");
  await mkdir(artifactDir);

  const fixtureFiles = [
    {
      artifact: "anarlog-gpui-stable-macos-silicon",
      subdir: "dmg",
      name: `Anarlog_${version}_aarch64.dmg`,
      content: Buffer.from("macos-silicon-dmg"),
      signed: false,
      sha256Sidecar: false,
    },
    {
      artifact: "anarlog-gpui-stable-macos-silicon",
      subdir: "macos",
      name: "Anarlog.app.tar.gz",
      content: Buffer.from("macos-silicon-updater"),
      signed: true,
      sha256Sidecar: false,
    },
    {
      artifact: "anarlog-gpui-stable-macos-intel",
      subdir: "dmg",
      name: `Anarlog_${version}_x64.dmg`,
      content: Buffer.from("macos-intel-dmg"),
      signed: false,
      sha256Sidecar: false,
    },
    {
      artifact: "anarlog-gpui-stable-macos-intel",
      subdir: "macos",
      name: "Anarlog.app.tar.gz",
      content: Buffer.from("macos-intel-updater"),
      signed: true,
      sha256Sidecar: false,
    },
    {
      artifact: "anarlog-gpui-stable-linux-x64",
      subdir: "appimage",
      name: `Anarlog_${version}_amd64.AppImage`,
      content: Buffer.from("linux-x64-appimage"),
      signed: true,
      sha256Sidecar: true,
    },
    {
      artifact: "anarlog-gpui-stable-linux-x64",
      subdir: "deb",
      name: `Anarlog_${version}_amd64.deb`,
      content: Buffer.from("linux-x64-deb"),
      signed: true,
      sha256Sidecar: true,
    },
    {
      artifact: "anarlog-gpui-stable-linux-arm64",
      subdir: "appimage",
      name: `Anarlog_${version}_aarch64.AppImage`,
      content: Buffer.from("linux-arm64-appimage"),
      signed: true,
      sha256Sidecar: true,
    },
    {
      artifact: "anarlog-gpui-stable-linux-arm64",
      subdir: "deb",
      name: `Anarlog_${version}_arm64.deb`,
      content: Buffer.from("linux-arm64-deb"),
      signed: true,
      sha256Sidecar: true,
    },
    {
      artifact: "anarlog-gpui-stable-windows-x64",
      subdir: "",
      name: `Anarlog_${version}_x64-setup.exe`,
      content: Buffer.from("windows-x64-nsis"),
      signed: true,
      sha256Sidecar: true,
    },
  ];

  const assets = [];
  const recordFile = async (artifact, dir, name, bytes) => {
    await writeFile(path.join(dir, name), bytes);
    assets.push({
      artifact,
      name,
      size: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  };

  for (const item of fixtureFiles) {
    const dir = item.subdir
      ? path.join(artifactDir, item.artifact, item.subdir)
      : path.join(artifactDir, item.artifact);
    await mkdir(dir, { recursive: true });
    await recordFile(item.artifact, dir, item.name, item.content);
    if (item.signed) {
      const sigBytes = Buffer.from(signer.signFile(item.content, item.name));
      await recordFile(item.artifact, dir, `${item.name}.sig`, sigBytes);
    }
    if (item.sha256Sidecar) {
      const shaHex = createHash("sha256").update(item.content).digest("hex");
      const sidecar = Buffer.from(`${shaHex}  ${item.name}\n`);
      await recordFile(item.artifact, dir, `${item.name}.sha256`, sidecar);
    }
  }

  const manifest = {
    schemaVersion: 1,
    app: "gpui",
    sourceWorkflow: ".github/workflows/desktop_cd.yaml",
    workflowRunId: "987654",
    runAttempt: "1",
    candidateSha,
    version,
    channel: "stable",
    assets,
  };

  return { root, artifactDir, tauriConfigPath, manifest, signer };
}

test("verifyGpuiArtifacts binds GPUI candidate files, updater signatures, and platform selections to provenance", async (t) => {
  const { root, artifactDir, tauriConfigPath, manifest } =
    await createGpuiFixture(t);

  // Verify both legacy ("Ed") and prehashed ("ED") minisign-verify test vectors.
  const vectorPubkey = Buffer.from(
    "untrusted comment: minisign public key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3",
    "utf8",
  ).toString("base64");
  const legacySig = Buffer.from(
    "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==",
    "utf8",
  ).toString("base64");
  const prehashedSig = Buffer.from(
    "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==",
    "utf8",
  ).toString("base64");
  const goodFile = path.join(root, "good.bin");
  const badFile = path.join(root, "bad.bin");
  await writeFile(goodFile, "test");
  await writeFile(badFile, "Test");
  for (const sig of [legacySig, prehashedSig]) {
    await verifyMinisignSignature({
      filePath: goodFile,
      signatureText: sig,
      pubkeyBase64: vectorPubkey,
    });
    await assert.rejects(
      verifyMinisignSignature({
        filePath: badFile,
        signatureText: sig,
        pubkeyBase64: vectorPubkey,
      }),
      /signature verification failed/,
    );
  }

  const verify = (overrides = {}) =>
    verifyGpuiArtifacts({
      manifest,
      artifactDir,
      version: "1.4.0",
      candidateSha,
      workflowRunId: "987654",
      tauriConfigPath,
      ...overrides,
    });

  const fullPlan = await verify();
  assert.equal(fullPlan.uploads.length, 9);
  const macOnlyPlan = await verify({
    includeLinux: false,
    includeWindows: false,
  });
  assert.equal(macOnlyPlan.uploads.length, 4);

  const windowsOnlyDir = path.join(root, "win-only");
  await mkdir(windowsOnlyDir);
  await mkdir(path.join(windowsOnlyDir, "anarlog-gpui-stable-windows-x64"));
  for (const entry of manifest.assets.filter(
    (item) => item.artifact === "anarlog-gpui-stable-windows-x64",
  )) {
    const src = path.join(
      artifactDir,
      "anarlog-gpui-stable-windows-x64",
      entry.name,
    );
    await writeFile(
      path.join(windowsOnlyDir, "anarlog-gpui-stable-windows-x64", entry.name),
      await readFile(src),
    );
  }
  const winPlan = await verify({
    artifactDir: windowsOnlyDir,
    artifactFilter: "anarlog-gpui-stable-windows-x64",
  });
  assert.equal(winPlan.uploads.length, 1);
  assert.equal(winPlan.uploads[0].publicPlatform, "nsis-x86_64");

  for (const [mutateManifest, error] of [
    [(m) => ({ ...m, app: "tauri" }), /app must be gpui/],
    [(m) => ({ ...m, channel: "staging" }), /channel must be stable/],
    [(m) => ({ ...m, runAttempt: "2" }), /unrerun workflow attempt/],
    [(m) => ({ ...m, candidateSha: "a".repeat(40) }), /candidate SHA mismatch/],
    [(m) => ({ ...m, workflowRunId: "111" }), /workflow run ID mismatch/],
    [(m) => ({ ...m, version: "1.4.1" }), /manifest version mismatch/],
  ]) {
    await assert.rejects(
      verify({ manifest: mutateManifest(structuredClone(manifest)) }),
      error,
    );
  }

  // Tampered .sig fails signature verification even if manifest hash matches disk.
  const tamperedSig = createMinisignSigner().signFile(
    Buffer.from("other"),
    "Anarlog.app.tar.gz",
  );
  const sigPath = path.join(
    artifactDir,
    "anarlog-gpui-stable-macos-silicon/macos/Anarlog.app.tar.gz.sig",
  );
  const originalSig = await readFile(sigPath);
  await writeFile(sigPath, tamperedSig);
  const sigManifest = structuredClone(manifest);
  const sigEntry = sigManifest.assets.find(
    (item) =>
      item.artifact === "anarlog-gpui-stable-macos-silicon" &&
      item.name === "Anarlog.app.tar.gz.sig",
  );
  sigEntry.size = Buffer.byteLength(tamperedSig);
  sigEntry.sha256 = createHash("sha256").update(tamperedSig).digest("hex");
  await assert.rejects(
    verify({ manifest: sigManifest }),
    /key ID does not match|signature verification failed/,
  );
  await writeFile(sigPath, originalSig);

  // Tampered payload byte fails manifest size/SHA-256 verification.
  const dmgPath = path.join(
    artifactDir,
    "anarlog-gpui-stable-macos-intel/dmg/Anarlog_1.4.0_x64.dmg",
  );
  const originalDmg = await readFile(dmgPath);
  await writeFile(dmgPath, "tampered-dmg");
  await assert.rejects(verify(), /size .* expected|SHA-256 mismatch/);
  await writeFile(dmgPath, originalDmg);

  // Mismatched .sha256 sidecar fails even if manifest recorded the wrong sidecar hash.
  const sidecarPath = path.join(
    artifactDir,
    "anarlog-gpui-stable-windows-x64/Anarlog_1.4.0_x64-setup.exe.sha256",
  );
  const badSidecar = `${"0".repeat(64)}  Anarlog_1.4.0_x64-setup.exe\n`;
  await writeFile(sidecarPath, badSidecar);
  const sidecarManifest = structuredClone(manifest);
  const sidecarEntry = sidecarManifest.assets.find(
    (item) =>
      item.artifact === "anarlog-gpui-stable-windows-x64" &&
      item.name.endsWith(".sha256"),
  );
  sidecarEntry.size = Buffer.byteLength(badSidecar);
  sidecarEntry.sha256 = createHash("sha256").update(badSidecar).digest("hex");
  await assert.rejects(
    verify({ manifest: sidecarManifest }),
    /does not match verified SHA-256/,
  );

  // Extra unrecorded file on disk fails manifest check.
  await writeFile(
    path.join(artifactDir, "anarlog-gpui-stable-windows-x64/extra.txt"),
    "surprise",
  );
  await assert.rejects(verify(), /do not match provenance manifest/);

  // Missing required Windows artifact fails when includeWindows is true.
  await rm(path.join(artifactDir, "anarlog-gpui-stable-windows-x64"), {
    recursive: true,
    force: true,
  });
  const noWinManifest = {
    ...structuredClone(manifest),
    assets: manifest.assets.filter(
      (item) => item.artifact !== "anarlog-gpui-stable-windows-x64",
    ),
  };
  await assert.rejects(
    verify({ manifest: noWinManifest, includeWindows: true }),
    /Missing downloaded GPUI artifact directory anarlog-gpui-stable-windows-x64/,
  );
});

test("verifyGpuiRelease rejects CrabNebula releases whose uploaded assets drift from verified GPUI provenance", async (t) => {
  const { root, artifactDir, tauriConfigPath, manifest } =
    await createGpuiFixture(t);
  const plan = await verifyGpuiArtifacts({
    manifest,
    artifactDir,
    version: "1.4.0",
    candidateSha,
    workflowRunId: "987654",
    tauriConfigPath,
  });

  const remoteAssetDir = path.join(root, "cdn-assets");
  await mkdir(remoteAssetDir);
  const release = {
    version: "1.4.0",
    status: "draft",
    assets: [],
  };
  for (const [index, upload] of plan.uploads.entries()) {
    const id = `cn-asset-${index}`;
    await writeFile(path.join(remoteAssetDir, id), await readFile(upload.file));
    release.assets.push({
      id,
      publicPlatform: upload.publicPlatform,
      updatePlatform: upload.updatePlatform,
      size: upload.size,
      signature: upload.signature,
    });
  }

  await verifyGpuiRelease({
    release,
    plan,
    version: "1.4.0",
    assetDir: remoteAssetDir,
  });

  for (const [mutate, error] of [
    [
      (r) => {
        r.version = "1.4.1";
      },
      /Release version is 1\.4\.1/,
    ],
    [
      (r) => {
        r.assets.find((asset) => asset.signature !== null).signature =
          "forged-signature";
      },
      /signature mismatch/,
    ],
    [
      (r) => {
        r.assets.pop();
      },
      /public platforms do not match/,
    ],
  ]) {
    const mutated = structuredClone(release);
    mutate(mutated);
    await assert.rejects(
      verifyGpuiRelease({
        release: mutated,
        plan,
        version: "1.4.0",
        assetDir: remoteAssetDir,
      }),
      error,
    );
  }

  await writeFile(
    path.join(remoteAssetDir, "cn-asset-0"),
    "x".repeat(plan.uploads[0].size),
  );
  await assert.rejects(
    verifyGpuiRelease({
      release,
      plan,
      version: "1.4.0",
      assetDir: remoteAssetDir,
    }),
    /SHA-256 mismatch/,
  );
});

test("GPUI stable publish workflow enforces bridge-gate cutover safety and release platform coverage", async (t) => {
  const workflow = await readFile(
    ".github/workflows/desktop_publish_gpui.yaml",
    "utf8",
  );

  // Verify gpuiArtifactSpecs matches releasePlatformPlan across every selection.
  for (const [includeLinux, includeWindows] of [
    [true, true],
    [true, false],
    [false, true],
    [false, false],
  ]) {
    const selected = Object.values(gpuiArtifactSpecs)
      .filter(
        (spec) =>
          spec.group === "macos" ||
          (spec.group === "linux" && includeLinux) ||
          (spec.group === "windows" && includeWindows),
      )
      .flatMap((spec) => spec.files);
    const expectedPublic = [
      ...releasePlatformPlan.macos.publicPlatforms,
      ...(includeLinux ? releasePlatformPlan.linux.publicPlatforms : []),
      ...(includeWindows ? releasePlatformPlan.windows.publicPlatforms : []),
    ].sort();
    const expectedUpdate = [
      ...releasePlatformPlan.macos.updatePlatforms,
      ...(includeLinux ? releasePlatformPlan.linux.updatePlatforms : []),
      ...(includeWindows ? releasePlatformPlan.windows.updatePlatforms : []),
    ].sort();
    assert.deepEqual(
      selected
        .map((file) => file.publicPlatform)
        .filter(Boolean)
        .sort(),
      expectedPublic,
    );
    assert.deepEqual(
      selected
        .map((file) => file.updatePlatform)
        .filter(Boolean)
        .sort(),
      expectedUpdate,
    );
  }

  const workflowCode = workflow.replace(/^\s*#.*$/gm, "");
  assert.match(workflow, /default:\s*["']?fastrepl\/anarlog-gpui["']?/);
  for (const forbidden of [
    /linux-package-bump/,
    /update-aur-pkgbuild/,
    /update-apt-repository/,
    /apps\/web\/public\/apt/,
    /packaging\/aur\/anarlog-bin/,
    /desktop_store_publish\.yaml/,
    /makeLatest:\s*true/,
    /ncipollo\/release-action/,
  ]) {
    assert.doesNotMatch(workflowCode, forbidden);
  }

  assert.match(workflow, /uses:\s*\$\/\.github\/actions\/cn_install/);
  const checkouts = [...workflow.matchAll(/uses:\s*actions\/checkout@/g)];
  const persistDisabled = [
    ...workflow.matchAll(/persist-credentials:\s*false/g),
  ];
  assert.ok(checkouts.length >= 3);
  assert.equal(persistDisabled.length, checkouts.length);
  assert.match(workflow, /verify-gpui\b/);
  assert.match(workflow, /verify-platforms\b/);
  assert.match(workflow, /verify-gpui-release\b/);
  assert.match(workflow, /runs-on:\s*windows-latest/);
  assert.match(workflow, /Get-AuthenticodeSignature/);
  assert.match(workflow, /WINDOWS_SIGNER_ORGANIZATION/);
  assert.match(workflow, /TimeStamperCertificate/);
  assert.match(
    workflow,
    /needs\.parse\.outputs\.include_windows != 'true' \|\| needs\.verify-windows-signature\.result == 'success'/,
  );

  // Execute the parse step directly to enforce input and target-app guards.
  const parseBlock = workflow.slice(
    workflow.indexOf("  parse:\n"),
    workflow.indexOf("\n  verify-candidate:\n"),
  );
  const runMatch = parseBlock.match(/        run: \|\n([\s\S]+)$/);
  assert.ok(runMatch, "missing parse script");
  const parseScript = runMatch[1].replace(/^          /gm, "");
  const scratchDir = await mkdtemp(path.join(os.tmpdir(), "gpui-parse-"));
  t.after(() => rm(scratchDir, { recursive: true, force: true }));

  const runParse = (overrides = {}) => {
    const githubOutput = path.join(scratchDir, "github-output");
    return spawnSync("bash", ["-e", "-c", parseScript], {
      encoding: "utf8",
      env: {
        PATH: process.env.PATH,
        GITHUB_OUTPUT: githubOutput,
        EXECUTION_REF: "refs/heads/main",
        EXECUTION_SHA: candidateSha,
        INPUT_VERSION: "1.4.0",
        INPUT_CANDIDATE_SHA: candidateSha,
        INPUT_DRY_RUN_ID: "123456",
        INPUT_CN_APPLICATION: "fastrepl/anarlog-gpui",
        INPUT_INCLUDE_LINUX: "true",
        INPUT_INCLUDE_WINDOWS: "true",
        ...overrides,
      },
    });
  };

  assert.equal(runParse().status, 0);
  for (const overrides of [
    { INPUT_CN_APPLICATION: "fastrepl/hyprnote2" },
    { INPUT_CN_APPLICATION: "FASTREPL/HYPRNOTE2" },
    { INPUT_CN_APPLICATION: "fastrepl" },
    { EXECUTION_REF: "refs/heads/feature" },
    { EXECUTION_SHA: "f".repeat(40) },
    { INPUT_VERSION: "1.4.0-staging.1" },
    { INPUT_DRY_RUN_ID: "0" },
  ]) {
    assert.notEqual(
      runParse(overrides).status,
      0,
      `expected parse failure for ${JSON.stringify(overrides)}`,
    );
  }
});
