import { spawn } from "node:child_process";
import { createSign } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const apiOrigin = "https://api.appstoreconnect.apple.com";
const editableVersionStates = new Set([
  "DEVELOPER_REJECTED",
  "METADATA_REJECTED",
  "PREPARE_FOR_SUBMISSION",
  "REJECTED",
]);
const submittedVersionStates = new Set([
  "ACCEPTED",
  "IN_REVIEW",
  "PENDING_APPLE_RELEASE",
  "PENDING_DEVELOPER_RELEASE",
  "PROCESSING_FOR_DISTRIBUTION",
  "READY_FOR_DISTRIBUTION",
  "READY_FOR_SALE",
  "WAITING_FOR_REVIEW",
]);

export class AppStoreConnectError extends Error {
  constructor(message, { status } = {}) {
    super(message);
    this.name = "AppStoreConnectError";
    this.status = status;
  }
}

function base64Url(value) {
  return Buffer.from(value).toString("base64url");
}

export function normalizePrivateKey(value) {
  let normalized = value.trim();
  if (normalized.startsWith('"') && normalized.endsWith('"')) {
    try {
      const parsed = JSON.parse(normalized);
      if (typeof parsed === "string") normalized = parsed.trim();
    } catch {}
  }
  return `${normalized.replaceAll("\\r\\n", "\n").replaceAll("\\n", "\n")}\n`;
}

export function createAppStoreConnectToken({
  issuerId,
  keyId,
  privateKey,
  now = Date.now,
}) {
  const issuedAt = Math.floor(now() / 1000);
  const header = base64Url(
    JSON.stringify({ alg: "ES256", kid: keyId, typ: "JWT" }),
  );
  const payload = base64Url(
    JSON.stringify({
      aud: "appstoreconnect-v1",
      exp: issuedAt + 19 * 60,
      iat: issuedAt,
      iss: issuerId,
    }),
  );
  const unsignedToken = `${header}.${payload}`;
  const signer = createSign("SHA256");
  signer.update(unsignedToken);
  signer.end();
  const signature = signer.sign({
    dsaEncoding: "ieee-p1363",
    key: privateKey,
  });

  return `${unsignedToken}.${base64Url(signature)}`;
}

function formatApiErrors(payload) {
  if (!Array.isArray(payload?.errors)) {
    return "App Store Connect returned an unexpected response";
  }

  return payload.errors
    .map((error) =>
      [error.status, error.code, error.title, error.detail]
        .filter(Boolean)
        .join(" "),
    )
    .join("; ");
}

export function createAppStoreConnectClient({
  fetchImpl = globalThis.fetch,
  issuerId,
  keyId,
  now,
  privateKey,
}) {
  return {
    async request(method, pathname, { body, query } = {}) {
      const url = new URL(pathname, apiOrigin);
      for (const [name, value] of Object.entries(query ?? {})) {
        if (value !== undefined) {
          url.searchParams.set(name, String(value));
        }
      }

      const response = await fetchImpl(url, {
        body: body === undefined ? undefined : JSON.stringify(body),
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${createAppStoreConnectToken({ issuerId, keyId, privateKey, now })}`,
          ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        },
        method,
      });
      const responseText = await response.text();
      const payload = responseText === "" ? null : JSON.parse(responseText);

      if (!response.ok) {
        throw new AppStoreConnectError(formatApiErrors(payload), {
          status: response.status,
        });
      }

      return payload;
    },
  };
}

async function resolveApp(client, bundleId) {
  const response = await client.request("GET", "/v1/apps", {
    query: { "filter[bundleId]": bundleId, limit: 2 },
  });
  if (response.data.length !== 1) {
    throw new Error(
      `Expected one App Store Connect app for ${bundleId}, found ${response.data.length}`,
    );
  }
  return response.data[0];
}

async function listMacVersions(client, appId) {
  const response = await client.request(
    "GET",
    `/v1/apps/${appId}/appStoreVersions`,
    {
      query: {
        "fields[appStoreVersions]":
          "appStoreState,platform,releaseType,versionString",
        "filter[platform]": "MAC_OS",
        limit: 200,
      },
    },
  );
  return response.data;
}

async function updateVersion(client, appStoreVersion, attributes) {
  const response = await client.request(
    "PATCH",
    `/v1/appStoreVersions/${appStoreVersion.id}`,
    {
      body: {
        data: {
          attributes,
          id: appStoreVersion.id,
          type: "appStoreVersions",
        },
      },
    },
  );
  return response.data;
}

async function ensureMacVersion(client, app, version) {
  const versions = await listMacVersions(client, app.id);
  const exactVersion = versions.find(
    (candidate) => candidate.attributes.versionString === version,
  );
  if (exactVersion) {
    if (submittedVersionStates.has(exactVersion.attributes.appStoreState)) {
      return { appStoreVersion: exactVersion, alreadySubmitted: true };
    }
    if (!editableVersionStates.has(exactVersion.attributes.appStoreState)) {
      throw new Error(
        `App Store version ${version} is in unsupported state ${exactVersion.attributes.appStoreState}`,
      );
    }
    if (exactVersion.attributes.releaseType !== "AFTER_APPROVAL") {
      return {
        appStoreVersion: await updateVersion(client, exactVersion, {
          releaseType: "AFTER_APPROVAL",
        }),
        alreadySubmitted: false,
      };
    }
    return { appStoreVersion: exactVersion, alreadySubmitted: false };
  }

  const placeholders = versions.filter(
    (candidate) =>
      candidate.attributes.appStoreState === "PREPARE_FOR_SUBMISSION",
  );
  if (placeholders.length > 1) {
    throw new Error(
      `Found ${placeholders.length} editable macOS App Store versions; cannot choose one safely`,
    );
  }
  if (placeholders.length === 1) {
    return {
      appStoreVersion: await updateVersion(client, placeholders[0], {
        releaseType: "AFTER_APPROVAL",
        versionString: version,
      }),
      alreadySubmitted: false,
    };
  }

  const response = await client.request("POST", "/v1/appStoreVersions", {
    body: {
      data: {
        attributes: {
          platform: "MAC_OS",
          releaseType: "AFTER_APPROVAL",
          versionString: version,
        },
        relationships: {
          app: { data: { id: app.id, type: "apps" } },
        },
        type: "appStoreVersions",
      },
    },
  });
  return { appStoreVersion: response.data, alreadySubmitted: false };
}

async function listBuilds(client, appId, version, buildVersion) {
  const response = await client.request("GET", "/v1/builds", {
    query: {
      "fields[builds]": "processingState,uploadedDate,version",
      "filter[app]": appId,
      "filter[version]": buildVersion,
      "filter[preReleaseVersion.platform]": "MAC_OS",
      "filter[preReleaseVersion.version]": version,
      limit: 10,
      sort: "-uploadedDate",
    },
  });
  return response.data;
}

async function waitForBuild({
  appId,
  buildVersion,
  client,
  pollIntervalMs,
  sleep,
  timeoutMs,
  version,
}) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const builds = await listBuilds(client, appId, version, buildVersion);
    const validBuild = builds.find(
      (build) => build.attributes.processingState === "VALID",
    );
    if (validBuild) {
      return validBuild;
    }

    const failedBuild = builds.find((build) =>
      ["FAILED", "INVALID"].includes(build.attributes.processingState),
    );
    if (failedBuild) {
      throw new Error(
        `App Store Connect rejected build ${failedBuild.attributes.version} with state ${failedBuild.attributes.processingState}`,
      );
    }
    await sleep(pollIntervalMs);
  }

  throw new Error(
    `Timed out waiting for App Store Connect to process macOS ${version} build ${buildVersion}`,
  );
}

async function attachBuild(client, appStoreVersionId, buildId) {
  await client.request(
    "PATCH",
    `/v1/appStoreVersions/${appStoreVersionId}/relationships/build`,
    { body: { data: { id: buildId, type: "builds" } } },
  );
}

function submissionContainsVersion(items, appStoreVersionId) {
  return items.some(
    (item) =>
      item.relationships?.appStoreVersion?.data?.id === appStoreVersionId,
  );
}

async function ensureReviewSubmission(client, appId, appStoreVersionId) {
  const submissionsResponse = await client.request(
    "GET",
    `/v1/apps/${appId}/reviewSubmissions`,
    {
      query: {
        "fields[reviewSubmissions]": "platform,state",
        "filter[platform]": "MAC_OS",
        "filter[state]": "READY_FOR_REVIEW,UNRESOLVED_ISSUES",
        limit: 10,
      },
    },
  );
  let submission = submissionsResponse.data[0];
  if (!submission) {
    const response = await client.request("POST", "/v1/reviewSubmissions", {
      body: {
        data: {
          attributes: { platform: "MAC_OS" },
          relationships: {
            app: { data: { id: appId, type: "apps" } },
          },
          type: "reviewSubmissions",
        },
      },
    });
    submission = response.data;
  }

  const itemsResponse = await client.request(
    "GET",
    `/v1/reviewSubmissions/${submission.id}/items`,
    { query: { limit: 50 } },
  );
  if (!submissionContainsVersion(itemsResponse.data, appStoreVersionId)) {
    await client.request("POST", "/v1/reviewSubmissionItems", {
      body: {
        data: {
          relationships: {
            appStoreVersion: {
              data: { id: appStoreVersionId, type: "appStoreVersions" },
            },
            reviewSubmission: {
              data: { id: submission.id, type: "reviewSubmissions" },
            },
          },
          type: "reviewSubmissionItems",
        },
      },
    });
  }

  await client.request("PATCH", `/v1/reviewSubmissions/${submission.id}`, {
    body: {
      data: {
        attributes: { submitted: true },
        id: submission.id,
        type: "reviewSubmissions",
      },
    },
  });
  return submission;
}

function runAltoolCommand(action, packagePath, keyId, issuerId) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "xcrun",
      [
        "altool",
        action,
        "--type",
        "macos",
        "--file",
        packagePath,
        "--apiKey",
        keyId,
        "--apiIssuer",
        issuerId,
      ],
      { stdio: "inherit" },
    );
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`xcrun altool exited with status ${code}`));
    });
  });
}

async function runAltool(packagePath, keyId, issuerId) {
  await runAltoolCommand("--validate-app", packagePath, keyId, issuerId);
  await runAltoolCommand("--upload-app", packagePath, keyId, issuerId);
}

export async function publishMacApp({
  buildVersion,
  bundleId,
  client,
  packagePath,
  pollIntervalMs = 30_000,
  sleep = (duration) => new Promise((resolve) => setTimeout(resolve, duration)),
  timeoutMs = 45 * 60_000,
  upload,
  version,
}) {
  const app = await resolveApp(client, bundleId);
  const versionResult = await ensureMacVersion(client, app, version);
  if (versionResult.alreadySubmitted) {
    return {
      appStoreVersionId: versionResult.appStoreVersion.id,
      alreadySubmitted: true,
    };
  }

  let builds = await listBuilds(client, app.id, version, buildVersion);
  if (builds.length === 0) {
    await upload(packagePath);
  }
  const build = await waitForBuild({
    appId: app.id,
    buildVersion,
    client,
    pollIntervalMs,
    sleep,
    timeoutMs,
    version,
  });
  await attachBuild(client, versionResult.appStoreVersion.id, build.id);
  const submission = await ensureReviewSubmission(
    client,
    app.id,
    versionResult.appStoreVersion.id,
  );

  return {
    alreadySubmitted: false,
    appStoreVersionId: versionResult.appStoreVersion.id,
    buildId: build.id,
    reviewSubmissionId: submission.id,
  };
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error(`Invalid argument ${name ?? ""}`.trim());
    }
    args[name.slice(2)] = value;
  }
  return args;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const required = [
    "build-version",
    "bundle-id",
    "issuer-id",
    "key-id",
    "package",
    "private-key",
    "version",
  ];
  const missing = required.filter((name) => !args[name]);
  if (missing.length > 0) {
    throw new Error(`Missing arguments: ${missing.join(", ")}`);
  }

  const privateKey = normalizePrivateKey(
    await readFile(args["private-key"], "utf8"),
  );
  await writeFile(args["private-key"], privateKey, { mode: 0o600 });
  const client = createAppStoreConnectClient({
    issuerId: args["issuer-id"],
    keyId: args["key-id"],
    privateKey,
  });
  const result = await publishMacApp({
    buildVersion: args["build-version"],
    bundleId: args["bundle-id"],
    client,
    packagePath: args.package,
    upload: (packagePath) =>
      runAltool(packagePath, args["key-id"], args["issuer-id"]),
    version: args.version,
  });
  console.log(JSON.stringify(result));
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
