// Builds Anarlog Camera.app: the helper, with the Camera Extension inside it.
//
//   node scripts/bundle.mjs [--debug] [--team-id TEAMID] [--identity "Developer ID Application: …"]
//                           [--app-profile app.provisionprofile] [--extension-profile ext.provisionprofile]
//                           [--version 1.4.2/1.4.2] [--host-bundle-id com.hyprnote.stable]
//                           [--icon path/to/icon.icns] [--out dir]
//
// Writes <out>/Anarlog Camera.app (default `bundle.noindex`, which keeps Spotlight from listing it).
// Without --identity it is ad-hoc signed; without --app-profile the helper is signed without
// `com.apple.developer.system-extension.install` (macOS kills a binary that claims it unprovisioned),
// so `render` and `serve`'s `check` work but activation fails with `missing_entitlement`. Without
// --team-id the extension's Mach service sits under a placeholder group, so it can't activate either.
// Nothing here installs or activates anything.
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** `CameraIdentity.helperBundleID` and `SinkGate.helperSigningID` in WatermarkKit. */
export const APP_ID = "so.anarlog.camera";
/** `CameraIdentity.extensionBundleID`. */
export const EXTENSION_ID = `${APP_ID}.extension`;
export const BUNDLE_NAME = "Anarlog Camera.app";
export const EXECUTABLE = "anarlog-camera";
/** Anarlog Camera's login agent: `AgentFiles.label` in WatermarkKit. */
export const AGENT_LABEL = `${APP_ID}.agent`;
/** Shown for unsigned dev builds, which can never activate. */
const PLACEHOLDER_TEAM = "UNSIGNED";
/** Anarlog, which the agent belongs to in Login Items. */
const DEFAULT_HOST_BUNDLE_ID = "com.hyprnote.stable";

const root = fileURLToPath(new URL("../", import.meta.url));
const defaultIcon = path.join(
  root,
  "../../apps/desktop/src-tauri/icons/stable/icon.icns",
);

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")} failed`);
}

function plist(entries) {
  const escape = (text) =>
    String(text)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  const value = (v) =>
    typeof v === "boolean"
      ? `<${v}/>`
      : Array.isArray(v)
        ? `<array>${v.map(value).join("")}</array>`
        : typeof v === "object"
          ? `<dict>${Object.entries(v)
              .map(([k, x]) => `<key>${escape(k)}</key>${value(x)}`)
              .join("")}</dict>`
          : `<string>${escape(v)}</string>`;
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">${value(entries)}</plist>
`;
}

/**
 * `CFBundleShortVersionString` and `CFBundleVersion` for both bundles, from `"short/build"`
 * (packaging passes Anarlog's own). sysextd replaces a registered extension only when the version
 * changes, so a dev build without one gets this second as its build number and every reinstall
 * replaces the last.
 */
export function bundleVersion(value, now = Date.now()) {
  const [short, build] = value?.split("/") ?? [];
  if (short && build)
    return { CFBundleShortVersionString: short, CFBundleVersion: build };
  return {
    CFBundleShortVersionString: "0.0.0",
    CFBundleVersion: String(Math.floor(now / 1000)),
  };
}

/** The App Group the extension's Mach service must sit under: `<team>.so.anarlog.camera`. */
export function appGroup(teamId) {
  return `${teamId ?? PLACEHOLDER_TEAM}.${APP_ID}`;
}

function checkTeamId(teamId) {
  if (teamId !== undefined && !/^[A-Z0-9]{10}$/.test(teamId))
    throw new Error(
      `--team-id must be a 10-character Apple team ID, got "${teamId}"`,
    );
}

export function bundle({
  debug = false,
  teamId,
  identity = "-",
  appProfile,
  extensionProfile,
  version: versionArg,
  hostBundleIds = [DEFAULT_HOST_BUNDLE_ID],
  icon = defaultIcon,
  out = path.join(root, "bundle.noindex"),
} = {}) {
  checkTeamId(teamId);
  if ((appProfile || extensionProfile || identity !== "-") && !teamId)
    throw new Error("signing with an identity or profiles needs --team-id");

  const configuration = debug ? "debug" : "release";
  run("swift", ["build", "-c", configuration]);
  const products = path.join(root, ".build", configuration);
  const group = appGroup(teamId);

  const app = path.join(out, BUNDLE_NAME, "Contents");
  const sysext = path.join(
    app,
    "Library/SystemExtensions",
    `${EXTENSION_ID}.systemextension`,
    "Contents",
  );
  rmSync(path.join(out, BUNDLE_NAME), { recursive: true, force: true });
  mkdirSync(path.join(app, "MacOS"), { recursive: true });
  mkdirSync(path.join(sysext, "MacOS"), { recursive: true });

  const version = bundleVersion(versionArg);
  const iconKeys = { CFBundleIconFile: "icon" };
  cpSync(
    path.join(products, "anarlog-camera-extension"),
    path.join(sysext, "MacOS", EXTENSION_ID),
  );
  writeFileSync(
    path.join(sysext, "Info.plist"),
    plist({
      CFBundleIdentifier: EXTENSION_ID,
      CFBundleExecutable: EXTENSION_ID,
      CFBundleName: "Anarlog Camera",
      CFBundleDisplayName: "Anarlog Camera",
      CFBundlePackageType: "SYSX",
      CFBundleInfoDictionaryVersion: "6.0",
      LSMinimumSystemVersion: "14.0",
      // The Mach service must sit under an App Group the extension belongs to.
      CMIOExtension: { CMIOExtensionMachServiceName: group },
      // sysextd refuses a Camera Extension without it (`validation_failed`).
      NSSystemExtensionUsageDescription:
        "Anarlog adds Anarlog Camera, which shows others that Anarlog is taking notes.",
      ...iconKeys,
      ...version,
    }),
  );

  cpSync(path.join(products, EXECUTABLE), path.join(app, "MacOS", EXECUTABLE));
  writeFileSync(
    path.join(app, "Info.plist"),
    plist({
      CFBundleIdentifier: APP_ID,
      CFBundleExecutable: EXECUTABLE,
      CFBundleName: "Anarlog Camera",
      CFBundleDisplayName: "Anarlog Camera",
      CFBundlePackageType: "APPL",
      CFBundleInfoDictionaryVersion: "6.0",
      LSMinimumSystemVersion: "14.0",
      LSBackgroundOnly: true,
      NSCameraUsageDescription:
        "Anarlog Camera shows your camera, with an Anarlog badge while Anarlog takes notes, in apps where you choose Anarlog Camera.",
      ...iconKeys,
      ...version,
    }),
  );
  // The login agent `anarlog-camera serve` registers while the camera is on (`enable_agent`): it
  // shows the person's camera whenever an app uses Anarlog Camera, with or without Anarlog open.
  mkdirSync(path.join(app, "Library", "LaunchAgents"), { recursive: true });
  writeFileSync(
    path.join(app, "Library", "LaunchAgents", `${AGENT_LABEL}.plist`),
    plist({
      Label: AGENT_LABEL,
      BundleProgram: `Contents/MacOS/${EXECUTABLE}`,
      ProgramArguments: [EXECUTABLE, "agent"],
      RunAtLoad: true,
      KeepAlive: true,
      // A camera frame is due every 33 ms: no background throttling.
      ProcessType: "Interactive",
      AssociatedBundleIdentifiers: hostBundleIds,
    }),
  );
  for (const contents of [sysext, app]) {
    mkdirSync(path.join(contents, "Resources"), { recursive: true });
    cpSync(icon, path.join(contents, "Resources", "icon.icns"));
  }

  const entitlementsDir = path.join(out, "entitlements");
  mkdirSync(entitlementsDir, { recursive: true });
  const provisioned = (bundleId) => ({
    "com.apple.application-identifier": `${teamId}.${bundleId}`,
    "com.apple.developer.team-identifier": teamId,
  });
  const extensionEntitlements = path.join(entitlementsDir, "extension.plist");
  writeFileSync(
    extensionEntitlements,
    plist({
      "com.apple.security.app-sandbox": true,
      "com.apple.security.application-groups": [group],
      ...(extensionProfile ? provisioned(EXTENSION_ID) : {}),
    }),
  );
  const appEntitlements = path.join(entitlementsDir, "app.plist");
  writeFileSync(
    appEntitlements,
    plist({
      "com.apple.security.device.camera": true,
      ...(appProfile
        ? {
            "com.apple.developer.system-extension.install": true,
            ...provisioned(APP_ID),
          }
        : {}),
    }),
  );
  if (extensionProfile)
    cpSync(extensionProfile, path.join(sysext, "embedded.provisionprofile"));
  if (appProfile)
    cpSync(appProfile, path.join(app, "embedded.provisionprofile"));

  const sign = (target, entitlements) => {
    const args = ["--force", "--sign", identity, "--options", "runtime"];
    args.push("--entitlements", entitlements);
    if (identity !== "-") args.push("--timestamp");
    run("codesign", [...args, target]);
  };
  // Inside out: the extension, then the app that holds it.
  sign(path.dirname(sysext), extensionEntitlements);
  sign(path.dirname(app), appEntitlements);
  run("codesign", ["--verify", "--deep", "--strict", path.dirname(app)]);

  const limits = [];
  if (identity === "-")
    limits.push("ad-hoc signed: the extension admits no sink writer");
  if (!teamId)
    limits.push(`no --team-id: Mach service under placeholder group ${group}`);
  if (!appProfile)
    limits.push("no --app-profile: activation fails with missing_entitlement");
  if (!extensionProfile)
    limits.push("no --extension-profile: the App Group isn't provisioned");
  for (const limit of limits) console.warn(`${BUNDLE_NAME}: ${limit}.`);
  return path.dirname(app);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const value = (flag) => {
    const index = args.indexOf(flag);
    return index >= 0 ? args[index + 1] : undefined;
  };
  const hosts =
    value("--host-bundle-id") ?? process.env.ANARLOG_CAMERA_HOST_BUNDLE_ID;
  console.log(
    bundle({
      debug: args.includes("--debug"),
      teamId: value("--team-id") ?? process.env.ANARLOG_CAMERA_TEAM_ID,
      identity:
        value("--identity") ?? process.env.ANARLOG_CAMERA_IDENTITY ?? "-",
      appProfile:
        value("--app-profile") ?? process.env.ANARLOG_CAMERA_APP_PROFILE,
      extensionProfile:
        value("--extension-profile") ??
        process.env.ANARLOG_CAMERA_EXTENSION_PROFILE,
      version: value("--version") ?? process.env.ANARLOG_CAMERA_VERSION,
      hostBundleIds: hosts ? hosts.split(",") : undefined,
      icon: value("--icon"),
      out: value("--out"),
    }),
  );
}
