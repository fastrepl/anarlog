import { spawn } from "node:child_process";
import { constants } from "node:fs";
import {
  access,
  readdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Libraries that must come from the host rather than the Ubuntu build box:
// Wayland, because host Mesa and WebKitGTK need the matching ABI; PipeWire and
// its SPA modules, because libpipewire loads plugins from a directory baked in
// at build time (Ubuntu's multiarch path does not exist on Fedora), so a bundled
// copy silently fails to capture there. libpulse stays bundled: it only speaks a
// stable socket protocol to the daemon, and it dlopens its own libpulsecommon.
const hostLibraryPatterns = [
  /^libwayland-.*\.so(?:\..*)?$/,
  /^libpipewire-.*\.so(?:\..*)?$/,
  /^libspa-.*\.so(?:\..*)?$/,
];
const gdkBackendDefault = 'export GDK_BACKEND="${GDK_BACKEND:-x11,wayland}"';

function patchGtkHook(source) {
  if (source.includes(gdkBackendDefault)) {
    return source;
  }

  // Keep X11 preferred, but allow Wayland-only sessions and explicit overrides.
  const forcedX11 = /^export GDK_BACKEND=x11(?:[ \t]+#.*)?$/m;
  if (!forcedX11.test(source)) {
    throw new Error("Unrecognized GDK_BACKEND setting in AppImage GTK hook");
  }
  return source.replace(forcedX11, gdkBackendDefault);
}

// The .deb's desktop entry starts /usr/bin/<app> by absolute path, so a CLI
// installed as ~/.local/bin/<app> (earlier on PATH) can't take over the menu
// launch. Tauri builds the AppImage's entry from the same template, and
// AppRun runs that Exec line, so the AppImage must name its bundled binary.
const debExecLine = /^Exec=\/usr\/bin\/([^/\s]+)(.*)$/m;

export function patchDesktopEntry(source) {
  return source.replace(debExecLine, "Exec=$1$2");
}

async function findDesktopEntries(appDirectory) {
  const directory = path.join(appDirectory, "usr", "share", "applications");
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }
  return entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".desktop"))
    .map((entry) => path.join(directory, entry.name))
    .sort();
}

async function findSingleBundleEntry(bundleDirectory, suffix, isExpectedType) {
  const entries = await readdir(bundleDirectory, { withFileTypes: true });
  const matches = entries.filter(
    (entry) => entry.name.endsWith(suffix) && isExpectedType(entry),
  );

  if (matches.length !== 1) {
    throw new Error(
      `Expected exactly one ${suffix} in ${bundleDirectory}, found ${matches.length}`,
    );
  }

  return path.join(bundleDirectory, matches[0].name);
}

export async function findBundledHostLibraries(directory) {
  const matches = [];

  async function walk(currentDirectory) {
    const entries = await readdir(currentDirectory, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = path.join(currentDirectory, entry.name);
      if (entry.isDirectory()) {
        await walk(entryPath);
      } else if (
        hostLibraryPatterns.some((pattern) => pattern.test(entry.name))
      ) {
        matches.push(entryPath);
      }
    }
  }

  await walk(directory);
  return matches.sort();
}

function defaultPluginPath() {
  const cacheDirectory =
    process.env.XDG_CACHE_HOME ?? path.join(os.homedir(), ".cache");
  return path.join(
    cacheDirectory,
    "tauri",
    "linuxdeploy-plugin-appimage.AppImage",
  );
}

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      env: options.env ?? process.env,
      stdio: "inherit",
    });

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(
        new Error(
          `${command} exited with ${signal ? `signal ${signal}` : `code ${code}`}`,
        ),
      );
    });
  });
}

export async function repackLinuxAppImage({
  bundleDirectory,
  arch = process.env.ARCH,
  pluginPath = process.env.TAURI_APPIMAGE_PLUGIN ?? defaultPluginPath(),
  run = runCommand,
}) {
  const appDirectory = await findSingleBundleEntry(
    bundleDirectory,
    ".AppDir",
    (entry) => entry.isDirectory(),
  );
  const appImage = await findSingleBundleEntry(
    bundleDirectory,
    ".AppImage",
    (entry) => entry.isFile(),
  );
  const hostLibraries = await findBundledHostLibraries(
    path.join(appDirectory, "usr"),
  );
  const gtkHook = path.join(
    appDirectory,
    "apprun-hooks",
    "linuxdeploy-plugin-gtk.sh",
  );
  const originalGtkHook = await readFile(gtkHook, "utf8");
  const patchedGtkHook = patchGtkHook(originalGtkHook);
  const updatedGtkHook = originalGtkHook !== patchedGtkHook;
  const desktopEntries = [];
  for (const entry of await findDesktopEntries(appDirectory)) {
    const original = await readFile(entry, "utf8");
    const patched = patchDesktopEntry(original);
    if (patched !== original) desktopEntries.push({ entry, patched });
  }
  const updatedDesktopEntries = desktopEntries.map(({ entry }) => entry);

  if (
    hostLibraries.length === 0 &&
    !updatedGtkHook &&
    updatedDesktopEntries.length === 0
  ) {
    console.log(
      `AppImage already uses host Wayland and PipeWire libraries and GTK backends: ${appImage}`,
    );
    return {
      appDirectory,
      appImage,
      removedLibraries: [],
      updatedGtkHook,
      updatedDesktopEntries,
    };
  }

  if (!arch) {
    throw new Error("AppImage architecture is required for repacking");
  }

  try {
    await access(pluginPath, constants.X_OK);
  } catch {
    throw new Error(`AppImage output plugin is not executable: ${pluginPath}`);
  }

  if (updatedGtkHook) {
    await writeFile(gtkHook, patchedGtkHook);
  }
  for (const { entry, patched } of desktopEntries) {
    await writeFile(entry, patched);
  }
  await Promise.all(hostLibraries.map((library) => rm(library)));

  const remainingLibraries = await findBundledHostLibraries(
    path.join(appDirectory, "usr"),
  );
  if (remainingLibraries.length > 0) {
    throw new Error(
      `Failed to remove bundled host libraries: ${remainingLibraries.join(", ")}`,
    );
  }

  console.log(`Removed bundled host libraries:\n${hostLibraries.join("\n")}`);

  await rm(appImage);
  await run(
    pluginPath,
    ["--appimage-extract-and-run", "--appdir", appDirectory],
    {
      env: {
        ...process.env,
        APPIMAGE_EXTRACT_AND_RUN: "1",
        ARCH: arch,
        OUTPUT: appImage,
      },
    },
  );

  const appImageStat = await stat(appImage);
  if (appImageStat.size === 0) {
    throw new Error(`Repacked AppImage is empty: ${appImage}`);
  }

  const signature = `${appImage}.sig`;
  await rm(signature, { force: true });
  await run("pnpm", ["-F", "desktop", "tauri", "signer", "sign", appImage]);

  const signatureStat = await stat(signature);
  if (signatureStat.size === 0) {
    throw new Error(`Regenerated AppImage signature is empty: ${signature}`);
  }

  console.log(`Repacked and re-signed AppImage: ${appImage}`);
  return {
    appDirectory,
    appImage,
    removedLibraries: hostLibraries,
    updatedGtkHook,
    updatedDesktopEntries,
  };
}

async function main() {
  const bundleDirectory = process.argv[2];
  if (!bundleDirectory) {
    throw new Error(
      "Usage: node scripts/repack-linux-appimage.mjs <appimage-bundle-directory>",
    );
  }

  await repackLinuxAppImage({
    bundleDirectory: path.resolve(bundleDirectory),
  });
}

if (fileURLToPath(import.meta.url) === path.resolve(process.argv[1] ?? "")) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
