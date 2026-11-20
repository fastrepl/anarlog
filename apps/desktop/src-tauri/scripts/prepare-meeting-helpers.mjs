// Qualification packaging for the native meeting helpers (macOS direct distribution only).
//
//   TAURI_CONF_PATH=src-tauri/tauri.conf.stable.json node src-tauri/scripts/prepare-meeting-helpers.mjs
//   pnpm -F desktop tauri build --config "$TAURI_CONF_PATH" ... --config ./src-tauri/tauri.conf.meeting-helpers.json
//
// The App Store build never uses this and bundles none of these helpers.
//
// Required: TAURI_CONF_PATH, the channel config passed to `tauri build` (relative to apps/desktop).
//   Its identifier picks the channel. The helper is appended to its bundle.externalBin, the way
//   scripts/sidecar.sh adds the other sidecars: `--config` replaces arrays, so the overlay can't.
// Optional: TAURI_ENV_TARGET_TRIPLE (defaults to the host's).
// Extension: ANARLOG_BROWSER_MEETING_EXTENSION_KEY, the channel's public key (base64 DER
//   SubjectPublicKeyInfo, Chrome's manifest `key`), pins the extension id. When
//   ANARLOG_BROWSER_MEETING_EXTENSION_ID is also set, the key must produce that id.
// Camera (all or none): APPLE_TEAM_ID, DEVELOPER_ID_APPLICATION, ANARLOG_CAMERA_APP_PROFILE,
//   ANARLOG_CAMERA_EXTENSION_PROFILE, ANARLOG_CAMERA_VERSION ("<short>/<build>").
import { execFileSync } from "node:child_process";
import { createHash, createPublicKey } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const srcTauri = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const desktop = path.dirname(srcTauri);
const root = path.resolve(desktop, "../..");
const out = path.join(root, "target", "meeting-helpers");

export const HELPER_BIN = "binaries/anarlog-meeting-capture";
const NATIVE_HOST_BIN = "binaries/char-chrome-native-host";
export const APP_STORE_IDENTIFIER = "com.hyprnote.desktop";
// Must match `host_name` in apps/desktop/src-tauri/src/meeting_browser.rs.
const CHANNELS = {
  "com.hyprnote.stable": { host: "so.anarlog.browser_meeting", icon: "stable" },
  "com.hyprnote.nightly": {
    host: "so.anarlog.nightly.browser_meeting",
    icon: "staging",
  },
  "com.hyprnote.staging": {
    host: "so.anarlog.staging.browser_meeting",
    icon: "staging",
  },
  "com.hyprnote.dev": { host: "so.anarlog.dev.browser_meeting", icon: "dev" },
};
export const CAMERA_INPUTS = [
  "APPLE_TEAM_ID",
  "DEVELOPER_ID_APPLICATION",
  "ANARLOG_CAMERA_APP_PROFILE",
  "ANARLOG_CAMERA_EXTENSION_PROFILE",
  "ANARLOG_CAMERA_VERSION",
];

export function channelFor(identifier) {
  if (identifier === APP_STORE_IDENTIFIER) {
    throw new Error(
      "the App Store build bundles no meeting helpers; use a direct-distribution channel config",
    );
  }
  const channel = CHANNELS[identifier];
  if (!channel) {
    throw new Error(`no meeting-helper channel for identifier "${identifier}"`);
  }
  return channel;
}

/**
 * Chrome's id for an extension whose manifest `key` is `key`: the first 128 bits of the SHA-256 of
 * the DER public key, one letter a-p per hex digit. Only a base64 RSA SubjectPublicKeyInfo is
 * accepted, so a private key can never end up in the bundled manifest.
 */
export function extensionIdFromKey(key) {
  const der = Buffer.from(key.trim(), "base64");
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(key.trim()) || der.length === 0) {
    throw new Error(
      "ANARLOG_BROWSER_MEETING_EXTENSION_KEY must be base64 DER, without PEM headers",
    );
  }
  let publicKey;
  try {
    publicKey = createPublicKey({ key: der, format: "der", type: "spki" });
  } catch {
    throw new Error(
      "ANARLOG_BROWSER_MEETING_EXTENSION_KEY is not an RSA public key (SubjectPublicKeyInfo); never pass the private key",
    );
  }
  if (publicKey.asymmetricKeyType !== "rsa") {
    throw new Error("ANARLOG_BROWSER_MEETING_EXTENSION_KEY must be an RSA key");
  }
  // OpenSSL accepts trailing bytes after a valid key. Never copy those bytes into the manifest.
  if (!publicKey.export({ format: "der", type: "spki" }).equals(der)) {
    throw new Error(
      "ANARLOG_BROWSER_MEETING_EXTENSION_KEY must contain only the DER public key",
    );
  }
  return [...createHash("sha256").update(der).digest("hex").slice(0, 32)]
    .map((digit) => String.fromCharCode(97 + parseInt(digit, 16)))
    .join("");
}

/** The extension key and id, checked against each other; null when no key is given. */
export function pinnedExtension(env) {
  const key = env.ANARLOG_BROWSER_MEETING_EXTENSION_KEY;
  const expected = env.ANARLOG_BROWSER_MEETING_EXTENSION_ID;
  if (!key) {
    if (expected) {
      throw new Error(
        "ANARLOG_BROWSER_MEETING_EXTENSION_ID needs ANARLOG_BROWSER_MEETING_EXTENSION_KEY to pin it",
      );
    }
    return null;
  }
  const id = extensionIdFromKey(key);
  if (expected && expected !== id) {
    throw new Error(
      `ANARLOG_BROWSER_MEETING_EXTENSION_KEY gives extension id ${id}, not ${expected}`,
    );
  }
  return { key: key.trim(), id };
}

export function writeNativeHostConfig(extensionDir, channel, extension) {
  const file = path.join(extensionDir, "native-host.json");
  if (!extension) {
    rmSync(file, { force: true });
    return;
  }
  writeFileSync(
    file,
    `${JSON.stringify({ version: 1, extensionId: extension.id, nativeHost: channel.host }, null, 2)}\n`,
  );
}

/** The camera's signing inputs, checked before anything builds; missing names when incomplete. */
export function cameraSigning(env) {
  const missing = CAMERA_INPUTS.filter((name) => !env[name]);
  if (missing.length > 0) {
    return { missing };
  }
  const team = env.APPLE_TEAM_ID;
  if (!/^[A-Z0-9]{10}$/.test(team)) {
    throw new Error(`APPLE_TEAM_ID must be a 10-character team ID`);
  }
  const identity = env.DEVELOPER_ID_APPLICATION;
  const named = /^Developer ID Application: .+ \(([A-Z0-9]{10})\)$/.exec(
    identity,
  );
  if (named ? named[1] !== team : !/^[0-9A-F]{40}$/i.test(identity)) {
    throw new Error(
      `DEVELOPER_ID_APPLICATION must be a "Developer ID Application: … (${team})" identity or its SHA-1`,
    );
  }
  for (const name of [
    "ANARLOG_CAMERA_APP_PROFILE",
    "ANARLOG_CAMERA_EXTENSION_PROFILE",
  ]) {
    const file = env[name];
    if (
      !file.endsWith(".provisionprofile") ||
      !existsSync(file) ||
      !statSync(file).isFile()
    ) {
      throw new Error(
        `${name} must name a Developer ID .provisionprofile file`,
      );
    }
  }
  // sysextd replaces an installed extension only when this changes, so a fallback is never used.
  if (!/^\d+\.\d+\.\d+\/\d+(\.\d+){0,2}$/.test(env.ANARLOG_CAMERA_VERSION)) {
    throw new Error(
      'ANARLOG_CAMERA_VERSION must be "<major.minor.patch>/<numeric build>"',
    );
  }
  return {
    missing: [],
    args: [
      "--team-id",
      team,
      "--identity",
      identity,
      "--app-profile",
      env.ANARLOG_CAMERA_APP_PROFILE,
      "--extension-profile",
      env.ANARLOG_CAMERA_EXTENSION_PROFILE,
      "--version",
      env.ANARLOG_CAMERA_VERSION,
    ],
  };
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function main(env) {
  if (process.platform !== "darwin") {
    throw new Error("macOS only");
  }
  if (!env.TAURI_CONF_PATH) {
    throw new Error("TAURI_CONF_PATH is required");
  }
  const channelConfigPath = path.resolve(desktop, env.TAURI_CONF_PATH);
  const channelConfig = readJson(channelConfigPath);
  const identifier =
    channelConfig.identifier ??
    readJson(path.join(srcTauri, "tauri.conf.json")).identifier;
  const channel = channelFor(identifier);
  const extension = pinnedExtension(env);
  const camera = cameraSigning(env);

  const triple =
    env.TAURI_ENV_TARGET_TRIPLE ??
    (process.arch === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin");
  const arch = triple.startsWith("aarch64") ? "arm64" : "x86_64";

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  // 1. Meeting capture helper, a Tauri externalBin in Contents/MacOS signed with Anarlog's
  // entitlements.
  const capture = path.join(root, "tools", "meeting-capture");
  const swiftBuild = [
    "build",
    "-c",
    "release",
    "--arch",
    arch,
    "--package-path",
    capture,
  ];
  execFileSync(
    "swift",
    [...swiftBuild, "--product", "anarlog-meeting-capture"],
    {
      stdio: "inherit",
    },
  );
  const built = execFileSync("swift", [...swiftBuild, "--show-bin-path"])
    .toString()
    .trim();
  mkdirSync(path.join(srcTauri, "binaries"), { recursive: true });
  cpSync(
    path.join(built, "anarlog-meeting-capture"),
    path.join(srcTauri, "binaries", `anarlog-meeting-capture-${triple}`),
  );
  const externalBin = new Set(channelConfig.bundle?.externalBin ?? []);
  externalBin.add(NATIVE_HOST_BIN).add(HELPER_BIN);
  channelConfig.bundle = {
    ...channelConfig.bundle,
    externalBin: [...externalBin].sort(),
  };
  writeFileSync(
    channelConfigPath,
    `${JSON.stringify(channelConfig, null, 2)}\n`,
  );

  // 2. Meet extension with this channel's native host name and, when given, its pinned key.
  const extensionDir = path.join(out, "browser-meeting-extension");
  cpSync(
    path.join(root, "browser-meeting-extension", "extension"),
    extensionDir,
    { recursive: true },
  );
  writeFileSync(
    path.join(extensionDir, "config.js"),
    `globalThis.AnarlogMeetingConfig = { nativeHost: ${JSON.stringify(channel.host)} };\n`,
  );
  if (extension) {
    const manifestPath = path.join(extensionDir, "manifest.json");
    writeFileSync(
      manifestPath,
      `${JSON.stringify({ ...readJson(manifestPath), key: extension.key }, null, 2)}\n`,
    );
    console.log(`[meeting-helpers] extension id ${extension.id}`);
  }
  writeNativeHostConfig(extensionDir, channel, extension);

  // 3. Anarlog Camera, nested in Contents/Helpers with its own signature and profiles.
  if (camera.missing.length === 0) {
    const icon = path.join(srcTauri, "icons", channel.icon, "icon.icns");
    execFileSync(
      process.execPath,
      [
        path.join(root, "tools", "camera-watermark", "scripts", "bundle.mjs"),
        ...camera.args,
        "--host-bundle-id",
        identifier,
        ...(existsSync(icon) ? ["--icon", icon] : []),
        "--out",
        out,
      ],
      { stdio: "inherit" },
    );
  }

  const unpinned = extension
    ? []
    : ["ANARLOG_BROWSER_MEETING_EXTENSION_KEY (extension id not pinned)"];
  for (const input of unpinned) {
    console.warn(`[meeting-helpers] missing ${input}`);
  }
  if (camera.missing.length > 0) {
    // An unsigned camera cannot activate; report it rather than bundling one that looks ready.
    console.error(
      `[meeting-helpers] camera not bundled, missing: ${camera.missing.join(", ")}`,
    );
    process.exit(2);
  }
  console.log(`[meeting-helpers] ready in ${out}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.env);
  } catch (error) {
    console.error(`[meeting-helpers] ${error.message}`);
    process.exit(1);
  }
}
