import type { Configuration } from "electron-builder";

// Channel shape mirrors `apps/desktop/src-tauri/tauri.conf.<channel>.json`:
// each channel is a distinct installable product with its own `appId`,
// executable name, deep-link scheme, icon set, and DMG chrome. The channel
// value is stamped into the packaged `package.json` via `extraMetadata` so
// `src/main/channel.ts` can read it back at runtime without needing a
// build-time bundler (we use `tsc`, not electron-vite).
//
// Auth for signing/notarization is env-driven:
//   - macOS code signing:    keychain identity (from `.github/actions/apple_cert`)
//                            or `CSC_LINK` / `CSC_KEY_PASSWORD`.
//   - macOS notarization:    `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`,
//                            `APPLE_TEAM_ID`. Disabled locally by default.

type Channel = "staging" | "nightly" | "stable";

function resolveChannel(): Channel {
  const raw = process.env.HYPR_CHANNEL;
  if (raw === "staging" || raw === "nightly" || raw === "stable") return raw;
  return "staging";
}

const channel = resolveChannel();

// Icon / DMG assets are still sourced from `apps/desktop/src-tauri/` until the
// PoC graduates to its own asset folder. Per-channel icons + DMG backgrounds
// already exist there for stable / nightly / staging.
const TAURI_ICON_BASE = "../desktop/src-tauri/icons";
const TAURI_ASSET_BASE = "../desktop/src-tauri/assets";

// Embedded `char` CLI is built per-arch by `cargo xtask prepare-desktop2-binaries`
// into `apps/desktop2/binaries/char-cli-<triple>`. electron-builder is invoked
// once per arch (see `desktop2_cd.yaml`); we resolve the right triple from the
// arch electron-builder is packaging so the binary that lands in
// `<App>.app/Contents/MacOS/char-cli` matches the app's architecture.
//
// Runtime resolver is `src/main/paths.ts::embeddedCliPath()`, which reads
// from `dirname(app.getPath("exe"))`. Install/uninstall + channel-scoped
// command name live in `src/main/embedded-cli.ts`.
//
// We deliberately use `mac.extraFiles` → `Contents/MacOS/` (not
// `extraResources` → `Contents/Resources/`) because:
//   - electron-builder auto-discovers + co-signs Mach-O files under
//     `Contents/MacOS/` with the app's Developer ID identity, so a single
//     notarization pass covers the CLI.
//   - `$PATH` symlink target under `MacOS/` matches Tauri's sidecar layout
//     (`apps/desktop/src-tauri/src/embedded_cli.rs` → `current_exe().parent()`).
function embeddedCliTriple(): string {
  // CI pins this explicitly per matrix arch; local `pnpm bundle` falls back to
  // the host architecture.
  const explicit = process.env.ELECTRON_BUILDER_ARCH;
  const arch = explicit ?? process.arch;
  if (arch === "arm64" || arch === "aarch64") return "aarch64-apple-darwin";
  if (arch === "x64" || arch === "x86_64") return "x86_64-apple-darwin";
  throw new Error(`Unsupported arch for embedded CLI packaging: ${arch}`);
}

const embeddedCliFile = `binaries/char-cli-${embeddedCliTriple()}`;

const base: Configuration = {
  copyright: "Copyright © 2026 Fastrepl",
  directories: {
    output: "release",
    buildResources: "build",
  },
  files: ["dist/ui/**/*", "dist/electron/**/*", "package.json"],
  asarUnpack: ["**/*.node"],
  extraResources: [
    {
      from: "../../plugins/tray/icons/tray_default.png",
      to: "plugins/tray/icons/tray_default.png",
    },
  ],
  // Stamped into the packaged `package.json`; read back by
  // `src/main/channel.ts` so the main process can self-identify.
  extraMetadata: {
    hyprChannel: channel,
  },
  mac: {
    category: "public.app-category.productivity",
    minimumSystemVersion: "14.2",
    darkModeSupport: true,
    hardenedRuntime: true,
    gatekeeperAssess: false,
    // notarytool via `@electron/notarize`; credentials from env.
    // Off locally by default, on in CI when `APPLE_ID` is present.
    notarize: process.env.CI === "true" && !!process.env.APPLE_ID,
    entitlements: "build/entitlements.mac.plist",
    entitlementsInherit: "build/entitlements.mac.plist",
    target: [{ target: "dmg", arch: ["arm64", "x64"] }],
    // See `embeddedCliTriple()` above for the rationale behind
    // `Contents/MacOS/` vs `Contents/Resources/`.
    extraFiles: [
      {
        from: embeddedCliFile,
        to: "MacOS/char-cli",
      },
    ],
    extendInfo: {
      LSApplicationCategoryType: "public.app-category.productivity",
      NSMicrophoneUsageDescription:
        "Char needs microphone access to capture meeting audio.",
      NSCalendarsUsageDescription:
        "Char reads your calendar to surface upcoming meetings.",
      NSContactsUsageDescription:
        "Char reads your contacts to attribute meeting participants.",
    },
  },
  dmg: {
    title: "${productName} ${version}",
    window: { width: 660, height: 430 },
    contents: [
      { x: 177, y: 200, type: "file" },
      { x: 483, y: 200, type: "link", path: "/Applications" },
    ],
  },
  win: {
    target: ["portable"],
    // Windows code-signing lives under `signtoolOptions` in electron-builder
    // v26; wire `publisherName` + a `sign` hook there when we set up signing.
  },
  linux: {
    target: ["deb", "appimage"],
    category: "Office",
    synopsis: "Hypr desktop client (Electron)",
    maintainer: "Fastrepl <team@char.com>",
  },
};

// Per-channel deltas. Anything omitted here falls through from `base`.
const perChannel: Record<Channel, Partial<Configuration>> = {
  stable: {
    appId: "com.char.stable",
    productName: "Char",
    executableName: "char",
    artifactName: "char-${version}-${arch}.${ext}",
    protocols: [{ name: "Char", schemes: ["hyprnote", "char"] }],
    mac: {
      ...base.mac,
      icon: `${TAURI_ICON_BASE}/stable/icon.icns`,
    },
    dmg: {
      ...base.dmg,
      background: `${TAURI_ASSET_BASE}/dmg-background-stable.png`,
    },
    win: {
      ...base.win,
      icon: `${TAURI_ICON_BASE}/stable/icon.ico`,
    },
    linux: {
      ...base.linux,
      icon: `${TAURI_ICON_BASE}/stable/icon.png`,
    },
  },
  nightly: {
    appId: "com.char.nightly",
    productName: "Char Nightly",
    executableName: "char-nightly",
    artifactName: "char-nightly-${version}-${arch}.${ext}",
    protocols: [{ name: "Char Nightly", schemes: ["hyprnote-nightly"] }],
    mac: {
      ...base.mac,
      icon: `${TAURI_ICON_BASE}/nightly/icon.icns`,
    },
    dmg: {
      ...base.dmg,
      background: `${TAURI_ASSET_BASE}/dmg-background-nightly.png`,
    },
    win: {
      ...base.win,
      icon: `${TAURI_ICON_BASE}/nightly/icon.ico`,
    },
    linux: {
      ...base.linux,
      icon: `${TAURI_ICON_BASE}/nightly/icon.png`,
    },
  },
  staging: {
    appId: "com.char.staging",
    productName: "Char Staging",
    executableName: "char-staging",
    artifactName: "char-staging-${version}-${arch}.${ext}",
    protocols: [{ name: "Char Staging", schemes: ["hyprnote-staging"] }],
    mac: {
      ...base.mac,
      icon: `${TAURI_ICON_BASE}/staging/icon.icns`,
    },
    dmg: {
      ...base.dmg,
      background: `${TAURI_ASSET_BASE}/dmg-background-staging.png`,
    },
    win: {
      ...base.win,
      icon: `${TAURI_ICON_BASE}/staging/icon.ico`,
    },
    linux: {
      ...base.linux,
      icon: `${TAURI_ICON_BASE}/staging/icon.png`,
    },
  },
};

const config: Configuration = {
  ...base,
  ...perChannel[channel],
};

export default config;
