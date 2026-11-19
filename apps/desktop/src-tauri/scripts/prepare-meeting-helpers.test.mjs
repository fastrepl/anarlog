import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  APP_STORE_IDENTIFIER,
  cameraSigning,
  channelFor,
  pinnedExtension,
  writeNativeHostConfig,
} from "./prepare-meeting-helpers.mjs";

const { publicKey, privateKey } = generateKeyPairSync("rsa", {
  modulusLength: 2048,
});
const publicDer = publicKey
  .export({ format: "der", type: "spki" })
  .toString("base64");

test("a private key is never accepted as the bundled extension key", () => {
  for (const key of [
    privateKey.export({ format: "der", type: "pkcs8" }).toString("base64"),
    privateKey.export({ format: "pem", type: "pkcs8" }),
    Buffer.concat([
      publicKey.export({ format: "der", type: "spki" }),
      privateKey.export({ format: "der", type: "pkcs8" }),
    ]).toString("base64"),
  ]) {
    assert.throws(
      () => pinnedExtension({ ANARLOG_BROWSER_MEETING_EXTENSION_KEY: key }),
      /EXTENSION_KEY/,
    );
  }
});

test("the bundled bridge identity matches its pinned extension and is removed when unpinned", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "meeting-bridge-"));
  try {
    const extension = pinnedExtension({
      ANARLOG_BROWSER_MEETING_EXTENSION_KEY: publicDer,
    });
    const channel = channelFor("com.hyprnote.stable");
    writeNativeHostConfig(dir, channel, extension);
    const config = path.join(dir, "native-host.json");
    assert.deepEqual(JSON.parse(readFileSync(config, "utf8")), {
      version: 1,
      extensionId: extension.id,
      nativeHost: "so.anarlog.browser_meeting",
    });
    writeNativeHostConfig(dir, channel, null);
    assert.equal(existsSync(config), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("an extension key that pins a different id than the app expects is refused", () => {
  const { id } = pinnedExtension({
    ANARLOG_BROWSER_MEETING_EXTENSION_KEY: publicDer,
  });
  assert.match(id, /^[a-p]{32}$/);
  assert.throws(
    () =>
      pinnedExtension({
        ANARLOG_BROWSER_MEETING_EXTENSION_KEY: publicDer,
        ANARLOG_BROWSER_MEETING_EXTENSION_ID: "a".repeat(32),
      }),
    new RegExp(`gives extension id ${id}`),
  );
});

test("the App Store build never gets meeting helpers", () => {
  assert.throws(() => channelFor(APP_STORE_IDENTIFIER), /App Store/);
});

test("a camera version sysextd can't order is refused instead of bundled", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "meeting-helpers-"));
  const profile = path.join(dir, "camera.provisionprofile");
  writeFileSync(profile, "");
  const env = {
    APPLE_TEAM_ID: "ABCDE12345",
    DEVELOPER_ID_APPLICATION: "Developer ID Application: Anarlog (ABCDE12345)",
    ANARLOG_CAMERA_APP_PROFILE: profile,
    ANARLOG_CAMERA_EXTENSION_PROFILE: profile,
  };
  assert.deepEqual(
    cameraSigning({ ...env, ANARLOG_CAMERA_VERSION: "1.4.2/1042" }).missing,
    [],
  );
  assert.throws(
    () => cameraSigning({ ...env, ANARLOG_CAMERA_VERSION: "1.4.2" }),
    /ANARLOG_CAMERA_VERSION/,
  );
});
