import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { changelog } from "../apps/desktop/plugins/changelog.ts";

test("stable and Nightly bundle their own release notes", async (t) => {
  for (const name of ["RELEASE_CHANNEL", "VITE_APP_VERSION"]) {
    const previous = process.env[name];
    t.after(() => {
      if (previous === undefined) delete process.env[name];
      else process.env[name] = previous;
    });
  }
  for (const [channel, version, file] of [
    ["stable", "1.4.23", "content/desktop/1.4.23.md"],
    ["nightly", "1.4.24-nightly.123", "nightly.md"],
  ]) {
    await t.test(channel, async () => {
      process.env.RELEASE_CHANNEL = channel;
      process.env.VITE_APP_VERSION = version;
      const load = changelog().load;
      const module = await load.call({}, "\0virtual:changelog");
      const content = readFileSync(
        new URL(`../packages/changelog/${file}`, import.meta.url),
        "utf8",
      );
      assert.ok(module.includes(JSON.stringify(content)));
      assert.ok(module.includes(`latestVersion = ${JSON.stringify(version)}`));
    });
  }
});
