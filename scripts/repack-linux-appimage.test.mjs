import assert from "node:assert/strict";
import { constants } from "node:fs";
import {
  access,
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

import { repackLinuxAppImage } from "./repack-linux-appimage.mjs";

async function createFixture(t, { withWaylandLibraries = true } = {}) {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-appimage-repack-"),
  );
  t.after(() => rm(directory, { force: true, recursive: true }));

  const appDirectory = path.join(directory, "Anarlog.AppDir");
  const libraryDirectory = path.join(appDirectory, "usr", "lib");
  const nestedLibraryDirectory = path.join(libraryDirectory, "gtk-3.0");
  const appImage = path.join(directory, "Anarlog_1.4.5_amd64.AppImage");
  const plugin = path.join(
    directory,
    "tools",
    "linuxdeploy-plugin-appimage.AppImage",
  );

  await mkdir(nestedLibraryDirectory, { recursive: true });
  await mkdir(path.dirname(plugin), { recursive: true });
  await writeFile(path.join(libraryDirectory, "libgtk-3.so.0"), "gtk");
  if (withWaylandLibraries) {
    await writeFile(
      path.join(libraryDirectory, "libwayland-client.so.0"),
      "wayland-client",
    );
    await writeFile(
      path.join(nestedLibraryDirectory, "libwayland-cursor.so.0.22.0"),
      "wayland-cursor",
    );
  }
  await writeFile(appImage, "original-appimage");
  await writeFile(`${appImage}.sig`, "stale-signature");
  await writeFile(plugin, "plugin");
  await chmod(plugin, 0o755);

  return { directory, appDirectory, appImage, plugin };
}

test("removes Wayland libraries, repacks, and regenerates the signature", async (t) => {
  const fixture = await createFixture(t);
  const commands = [];

  const result = await repackLinuxAppImage({
    arch: "x86_64",
    bundleDirectory: fixture.directory,
    pluginPath: fixture.plugin,
    run: async (command, args, options) => {
      commands.push({ command, args, options });
      if (command === fixture.plugin) {
        await assert.rejects(access(fixture.appImage, constants.F_OK));
        await writeFile(fixture.appImage, "repacked-appimage");
      } else {
        await writeFile(`${fixture.appImage}.sig`, "fresh-signature");
      }
    },
  });

  assert.equal(result.removedLibraries.length, 2);
  await assert.rejects(
    access(
      path.join(fixture.appDirectory, "usr", "lib", "libwayland-client.so.0"),
      constants.F_OK,
    ),
  );
  await assert.rejects(
    access(
      path.join(
        fixture.appDirectory,
        "usr",
        "lib",
        "gtk-3.0",
        "libwayland-cursor.so.0.22.0",
      ),
      constants.F_OK,
    ),
  );
  assert.equal(
    await readFile(
      path.join(fixture.appDirectory, "usr", "lib", "libgtk-3.so.0"),
      "utf8",
    ),
    "gtk",
  );
  assert.equal(await readFile(fixture.appImage, "utf8"), "repacked-appimage");
  assert.equal(
    await readFile(`${fixture.appImage}.sig`, "utf8"),
    "fresh-signature",
  );

  assert.deepEqual(commands[0].args, [
    "--appimage-extract-and-run",
    "--appdir",
    fixture.appDirectory,
  ]);
  assert.equal(commands[0].options.env.APPIMAGE_EXTRACT_AND_RUN, "1");
  assert.equal(commands[0].options.env.ARCH, "x86_64");
  assert.equal(commands[0].options.env.OUTPUT, fixture.appImage);
  assert.deepEqual(commands[1], {
    command: "pnpm",
    args: ["-F", "desktop", "tauri", "signer", "sign", fixture.appImage],
    options: undefined,
  });
});

test("keeps an already-clean AppImage and its signature unchanged", async (t) => {
  const fixture = await createFixture(t, { withWaylandLibraries: false });

  const result = await repackLinuxAppImage({
    bundleDirectory: fixture.directory,
    pluginPath: path.join(fixture.directory, "missing-plugin"),
    run: async () => {
      throw new Error("clean AppImages must not be repacked");
    },
  });

  assert.deepEqual(result.removedLibraries, []);
  assert.equal(await readFile(fixture.appImage, "utf8"), "original-appimage");
  assert.equal(
    await readFile(`${fixture.appImage}.sig`, "utf8"),
    "stale-signature",
  );
});

test("fails before mutation when the AppImage output plugin is unavailable", async (t) => {
  const fixture = await createFixture(t);
  const waylandLibrary = path.join(
    fixture.appDirectory,
    "usr",
    "lib",
    "libwayland-client.so.0",
  );

  await assert.rejects(
    repackLinuxAppImage({
      arch: "x86_64",
      bundleDirectory: fixture.directory,
      pluginPath: path.join(fixture.directory, "missing-plugin"),
    }),
    /AppImage output plugin is not executable/,
  );

  assert.equal(await readFile(waylandLibrary, "utf8"), "wayland-client");
  assert.equal(
    await readFile(`${fixture.appImage}.sig`, "utf8"),
    "stale-signature",
  );
});

test("refuses an ambiguous AppImage bundle before mutation", async (t) => {
  const fixture = await createFixture(t);
  await writeFile(
    path.join(fixture.directory, "Anarlog_1.4.5_arm64.AppImage"),
    "other-appimage",
  );

  await assert.rejects(
    repackLinuxAppImage({
      arch: "x86_64",
      bundleDirectory: fixture.directory,
      pluginPath: fixture.plugin,
    }),
    /Expected exactly one \.AppImage.*found 2/,
  );

  assert.equal(
    await readFile(
      path.join(fixture.appDirectory, "usr", "lib", "libwayland-client.so.0"),
      "utf8",
    ),
    "wayland-client",
  );
});

test("requires an architecture before mutating the AppDir", async (t) => {
  const fixture = await createFixture(t);
  const waylandLibrary = path.join(
    fixture.appDirectory,
    "usr",
    "lib",
    "libwayland-client.so.0",
  );

  await assert.rejects(
    repackLinuxAppImage({
      arch: "",
      bundleDirectory: fixture.directory,
      pluginPath: fixture.plugin,
    }),
    /AppImage architecture is required/,
  );

  assert.equal(await readFile(waylandLibrary, "utf8"), "wayland-client");
});
