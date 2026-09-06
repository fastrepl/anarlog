import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
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

async function createFixture(
  t,
  { withWaylandLibraries = true, patchedGtkHook = false } = {},
) {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-appimage-repack-"),
  );
  t.after(() => rm(directory, { force: true, recursive: true }));

  const appDirectory = path.join(directory, "Anarlog.AppDir");
  const libraryDirectory = path.join(appDirectory, "usr", "lib");
  const nestedLibraryDirectory = path.join(libraryDirectory, "gtk-3.0");
  const appImage = path.join(directory, "Anarlog_1.4.5_amd64.AppImage");
  const gtkHook = path.join(
    appDirectory,
    "apprun-hooks",
    "linuxdeploy-plugin-gtk.sh",
  );
  const plugin = path.join(
    directory,
    "tools",
    "linuxdeploy-plugin-appimage.AppImage",
  );

  await mkdir(nestedLibraryDirectory, { recursive: true });
  await mkdir(path.dirname(plugin), { recursive: true });
  await mkdir(path.dirname(gtkHook), { recursive: true });
  await writeFile(
    gtkHook,
    [
      "#!/usr/bin/env bash",
      "export GTK_DATA_PREFIX=unchanged",
      patchedGtkHook
        ? 'export GDK_BACKEND="${GDK_BACKEND:-x11,wayland}"'
        : "export GDK_BACKEND=x11 # Crash with Wayland backend on Wayland",
      "",
    ].join("\n"),
  );
  await chmod(gtkHook, 0o755);
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

  return { directory, appDirectory, appImage, plugin, gtkHook };
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
  assert.equal(result.updatedGtkHook, true);
  await access(fixture.gtkHook, constants.X_OK);
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
  const fixture = await createFixture(t, {
    withWaylandLibraries: false,
    patchedGtkHook: true,
  });

  const result = await repackLinuxAppImage({
    bundleDirectory: fixture.directory,
    pluginPath: path.join(fixture.directory, "missing-plugin"),
    run: async () => {
      throw new Error("clean AppImages must not be repacked");
    },
  });

  assert.deepEqual(result.removedLibraries, []);
  assert.equal(result.updatedGtkHook, false);
  assert.equal(await readFile(fixture.appImage, "utf8"), "original-appimage");
  assert.equal(
    await readFile(`${fixture.appImage}.sig`, "utf8"),
    "stale-signature",
  );
});

test("repacks a hook-only fix and preserves explicit GTK backend preferences", async (t) => {
  const fixture = await createFixture(t, { withWaylandLibraries: false });
  const commands = [];
  const result = await repackLinuxAppImage({
    arch: "x86_64",
    bundleDirectory: fixture.directory,
    pluginPath: fixture.plugin,
    run: async (command) => {
      commands.push(command);
      await writeFile(
        command === fixture.plugin
          ? fixture.appImage
          : `${fixture.appImage}.sig`,
        "regenerated",
      );
    },
  });
  assert.deepEqual(result.removedLibraries, []);
  assert.equal(result.updatedGtkHook, true);
  assert.deepEqual(commands, [fixture.plugin, "pnpm"]);

  for (const [preference, expected] of [
    [undefined, "x11,wayland"],
    ["", "x11,wayland"],
    ["wayland", "wayland"],
    ["x11", "x11"],
    ["wayland,x11", "wayland,x11"],
  ]) {
    const env = { ...process.env };
    delete env.GDK_BACKEND;
    if (preference !== undefined) env.GDK_BACKEND = preference;
    const backend = execFileSync(
      "bash",
      [
        "-c",
        '. "$1"; printf "%s" "$GDK_BACKEND:$GTK_DATA_PREFIX"',
        "bash",
        fixture.gtkHook,
      ],
      { env, encoding: "utf8" },
    );
    assert.equal(backend, `${expected}:unchanged`);
  }
});

test("rejects an unfamiliar GTK hook before mutating the bundle", async (t) => {
  const fixture = await createFixture(t);
  await writeFile(fixture.gtkHook, "export GDK_BACKEND=unknown\n");
  await assert.rejects(
    repackLinuxAppImage({
      arch: "x86_64",
      bundleDirectory: fixture.directory,
      pluginPath: fixture.plugin,
    }),
    /Unrecognized GDK_BACKEND/,
  );
  assert.equal(await readFile(fixture.appImage, "utf8"), "original-appimage");
  assert.equal(
    await readFile(
      path.join(fixture.appDirectory, "usr", "lib", "libwayland-client.so.0"),
      "utf8",
    ),
    "wayland-client",
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
  assert.match(await readFile(fixture.gtkHook, "utf8"), /GDK_BACKEND=x11 #/);
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
