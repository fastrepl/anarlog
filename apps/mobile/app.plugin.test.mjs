import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
import test from "node:test";

import withAnarlogAndroidArchitectures from "./app.plugin.js";

test("packages only Android architectures with a bundled CloudSync library", async () => {
  const vendor = new URL(
    "../../crates/cloudsync/vendor/cloudsync/android/",
    import.meta.url,
  );
  const supported = readdirSync(vendor).filter((abi) =>
    existsSync(new URL(`${abi}/cloudsync.so`, vendor)),
  );
  const preserved = [
    { type: "comment", value: "Keep project settings" },
    { type: "property", key: "newArchEnabled", value: "true" },
    { type: "property", key: "android.minSdkVersion", value: "24" },
  ];
  const config = withAnarlogAndroidArchitectures({
    name: "Anarlog",
    slug: "anarlog-mobile",
  });
  let result = {
    ...config,
    modRequest: {
      platform: "android",
      modName: "gradleProperties",
      projectRoot: process.cwd(),
    },
    modResults: [
      ...preserved,
      {
        type: "property",
        key: "reactNativeArchitectures",
        value: "x86,arm64-v8a",
      },
      { type: "property", key: "reactNativeArchitectures", value: "x86" },
    ],
  };
  for (let pass = 0; pass < 2; pass++) {
    result = await config.mods.android.gradleProperties(result);
    const architectures = result.modResults.filter(
      (item) => item.key === "reactNativeArchitectures",
    );
    assert.equal(architectures.length, 1);
    assert.deepEqual(
      architectures[0].value.split(",").sort(),
      supported.sort(),
    );
    assert.deepEqual(
      result.modResults.filter(
        (item) => item.key !== "reactNativeArchitectures",
      ),
      preserved,
    );
  }
});
