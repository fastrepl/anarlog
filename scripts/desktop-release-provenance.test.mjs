import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
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

test("accepts the complete desktop public and update platform sets", () => {
  verifyDesktopPlatformSets(createDesktopRelease());
});

test("accepts separate public and updater assets", () => {
  verifyDesktopPlatformSets(createDesktopRelease({ splitMappings: true }));
});

test("accepts a macOS and Linux release without Windows", () => {
  verifyDesktopPlatformSets(createDesktopRelease({ includeWindows: false }), {
    includeWindows: false,
  });
});

test("accepts a macOS-only release", () => {
  verifyDesktopPlatformSets(
    createDesktopRelease({ includeLinux: false, includeWindows: false }),
    { includeLinux: false, includeWindows: false },
  );
});

test("rejects an omitted selected platform", () => {
  assert.throws(
    () =>
      verifyDesktopPlatformSets(
        createDesktopRelease({ includeWindows: false }),
      ),
    /public platforms do not match/,
  );
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
    /public platforms do not match/,
  );
});

test("rejects duplicate public desktop platforms", () => {
  const release = createDesktopRelease();
  release.assets[0].publicPlatform = release.assets[2].publicPlatform;

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
    /update platforms do not match/,
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
    /must map to a public or update platform/,
  );
});

test("rejects an unsigned updater asset", () => {
  const release = createDesktopRelease();
  const updater = release.assets.find((asset) => asset.updatePlatform !== null);
  updater.signature = null;

  assert.throws(
    () => verifyDesktopPlatformSets(release),
    /updater asset must carry a signature/,
  );
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

test("accepts workflows that cover the complete release plan", () => {
  verifyWorkflowPlatformCoverage(workflowFixtures());
});

test("rejects a publish workflow that omits a planned platform", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  assert.throws(
    () =>
      verifyWorkflowPlatformCoverage({
        publishWorkflow: publishWorkflow.replace(
          /^.*platform: nsis-x86_64.*\n/m,
          "",
        ),
        cdWorkflow,
      }),
    /do not match the release plan/,
  );
});

test("allows a planned platform to be downloaded by more than one job", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  verifyWorkflowPlatformCoverage({
    publishWorkflow: `${publishWorkflow}          platform: dmg-aarch64\n`,
    cdWorkflow,
  });
});

test("rejects a publish workflow with an unknown platform download", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  assert.throws(
    () =>
      verifyWorkflowPlatformCoverage({
        publishWorkflow: `${publishWorkflow}          platform: msi-x86_64\n`,
        cdWorkflow,
      }),
    /do not match the release plan/,
  );
});

test("rejects a publish workflow with a renamed platform download", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  assert.throws(
    () =>
      verifyWorkflowPlatformCoverage({
        publishWorkflow: publishWorkflow.replace(
          "platform: debian-x86_64",
          "platform: deb-x86_64",
        ),
        cdWorkflow,
      }),
    /do not match the release plan/,
  );
});

test("rejects a release workflow that drops a planned build target", () => {
  const { publishWorkflow, cdWorkflow } = workflowFixtures();
  assert.throws(
    () =>
      verifyWorkflowPlatformCoverage({
        publishWorkflow,
        cdWorkflow: cdWorkflow.replace("x86_64-pc-windows-msvc", ""),
      }),
    /does not build the planned target x86_64-pc-windows-msvc/,
  );
});

test("repository release workflows match the authored release plan", async () => {
  const [publishWorkflow, cdWorkflow] = await Promise.all([
    readFile(".github/workflows/desktop_publish.yaml", "utf8"),
    readFile(".github/workflows/desktop_cd.yaml", "utf8"),
  ]);

  verifyWorkflowPlatformCoverage({ publishWorkflow, cdWorkflow });
});

test("stable desktop releases submit both store packages", async () => {
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
  assert.match(storePublishJob, /include_macos: true/);
  assert.match(storePublishJob, /include_windows: true/);
  assert.match(storePublishJob, /submit_to_stores: true/);
  assert.doesNotMatch(storePublishJob, /secrets: inherit/);
  assert.match(
    storeWorkflow,
    /node workflow-source\/scripts\/app-store-connect-submit\.mjs/,
  );
  assert.match(storeWorkflow, /Submitted to App Review/);

  const expectedSecrets = [
    "APPLE_TEAM_ID",
    "APPSTORE_API_KEY_ID",
    "APPSTORE_API_PRIVATE_KEY",
    "APPSTORE_ISSUER_ID",
    "AZURE_AD_APPLICATION_SECRET",
    "CN_API_KEY",
    "KEYCHAIN_PASSWORD",
    "MAC_APP_STORE_APPLICATION_CERTIFICATE",
    "MAC_APP_STORE_APPLICATION_CERTIFICATE_PASSWORD",
    "MAC_APP_STORE_INSTALLER_CERTIFICATE",
    "MAC_APP_STORE_INSTALLER_CERTIFICATE_PASSWORD",
    "MAC_APP_STORE_PROVISIONING_PROFILE",
    "POSTHOG_API_KEY",
    "SELLER_ID",
    "SENTRY_DSN_HYPRNOTE_2",
    "VITE_PRO_PRODUCT_ID",
    "VITE_SUPABASE_ANON_KEY",
    "VITE_SUPABASE_URL",
  ];
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

  assert.deepEqual(declaredSecrets, expectedSecrets);
  assert.deepEqual(forwardedSecrets, expectedSecrets);
});

test("Mac App Store builds include a compiled app icon catalog", async () => {
  const [storeWorkflow, appStoreConfig, stableConfig] = await Promise.all([
    readFile(".github/workflows/desktop_store_publish.yaml", "utf8"),
    readFile("apps/desktop/src-tauri/tauri.conf.app-store.json", "utf8").then(
      JSON.parse,
    ),
    readFile("apps/desktop/src-tauri/tauri.conf.stable.json", "utf8").then(
      JSON.parse,
    ),
  ]);

  assert.match(storeWorkflow, /name: Compile Mac App Store asset catalog/);
  assert.match(storeWorkflow, /xcrun actool/);
  assert.match(storeWorkflow, /icons\/stable\/icon\.icns/);
  assert.match(storeWorkflow, /AppIcon\.appiconset/);
  assert.match(storeWorkflow, /icon_512x512@2x\.png/);
  assert.doesNotMatch(storeWorkflow, /icons\/src\/stable\.icon/);
  assert.match(storeWorkflow, /Contents\/Resources\/Assets\.car/);
  assert.match(
    storeWorkflow,
    /security find-identity -v "\$RUNNER_TEMP\/mac-app-store\.keychain-db"/,
  );
  assert.doesNotMatch(
    storeWorkflow,
    /APPLICATION_CERT_SHA1:.*application-cert-sha1/,
  );
  assert.doesNotMatch(storeWorkflow, /application_cert_sha1\^\^/);
  assert.match(
    storeWorkflow,
    /\.bundle\.macOS\.files\["Resources\/Assets\.car"\]/,
  );
  assert.equal(
    appStoreConfig.bundle.macOS.files["Resources/Assets.car"],
    "./resources/app-store/Assets.car",
  );
  assert.equal(
    stableConfig.bundle.macOS.files["Resources/Assets.car"],
    undefined,
  );
});

test("keeps Mac App Store privacy metadata and replacement builds reviewable", async () => {
  const [entitlements, infoPlist, storeWorkflow, submitScript] =
    await Promise.all([
      readFile("apps/desktop/src-tauri/Entitlements.app-store.plist", "utf8"),
      readFile("apps/desktop/src-tauri/Info.plist", "utf8"),
      readFile(".github/workflows/desktop_store_publish.yaml", "utf8"),
      readFile("scripts/app-store-connect-submit.mjs", "utf8"),
    ]);

  assert.doesNotMatch(entitlements, /com\.apple\.security\.network\.server/);
  assert.match(infoPlist, /NSContactsUsageDescription/);
  assert.match(
    infoPlist,
    /match calendar attendees with names and email addresses saved on your Mac/,
  );
  assert.match(storeWorkflow, /build_number:/);
  assert.match(storeWorkflow, /bundleVersion = \$build/);
  assert.match(storeWorkflow, /--build-version "\$BUILD_NUMBER"/);
  assert.match(submitScript, /"filter\[version\]": buildVersion/);
  assert.match(
    submitScript,
    /"filter\[state\]": "READY_FOR_REVIEW,UNRESOLVED_ISSUES"/,
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
