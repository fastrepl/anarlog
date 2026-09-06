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

// Host Mesa and WebKitGTK must resolve against the host's matching Wayland ABI.
const waylandLibraryPattern = /^libwayland-.*\.so(?:\..*)?$/;
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

export async function findBundledWaylandLibraries(directory) {
  const matches = [];

  async function walk(currentDirectory) {
    const entries = await readdir(currentDirectory, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = path.join(currentDirectory, entry.name);
      if (entry.isDirectory()) {
        await walk(entryPath);
      } else if (waylandLibraryPattern.test(entry.name)) {
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
  const waylandLibraries = await findBundledWaylandLibraries(
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

  if (waylandLibraries.length === 0 && !updatedGtkHook) {
    console.log(
      `AppImage already uses host Wayland libraries and GTK backends: ${appImage}`,
    );
    return { appDirectory, appImage, removedLibraries: [], updatedGtkHook };
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
  await Promise.all(waylandLibraries.map((library) => rm(library)));

  const remainingLibraries = await findBundledWaylandLibraries(
    path.join(appDirectory, "usr"),
  );
  if (remainingLibraries.length > 0) {
    throw new Error(
      `Failed to remove bundled Wayland libraries: ${remainingLibraries.join(", ")}`,
    );
  }

  console.log(
    `Removed bundled Wayland libraries:\n${waylandLibraries.join("\n")}`,
  );

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
    removedLibraries: waylandLibraries,
    updatedGtkHook,
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
