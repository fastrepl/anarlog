import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import configureMobileApp, { resolveAppVariant } from "./app.config.ts";

const baseConfig = JSON.parse(
  readFileSync(new URL("./app.json", import.meta.url), "utf8"),
).expo;

const expectedVariants = {
  dev: {
    name: "Anarlog Dev",
    icon: "./assets/images/icon-dev.png",
    scheme: "anarlog-dev",
    bundleIdentifier: "so.anarlog.mobile.dev",
  },
  staging: {
    name: "Anarlog Staging",
    icon: "./assets/images/icon-staging.png",
    scheme: "anarlog-staging",
    bundleIdentifier: "so.anarlog.mobile.staging",
  },
  stable: {
    name: "Anarlog",
    icon: "./assets/images/icon.png",
    scheme: "anarlog",
    bundleIdentifier: "so.anarlog.mobile",
  },
};

test("configures distinct app identities for every build profile", () => {
  const originalVariant = process.env.APP_VARIANT;
  try {
    for (const [appVariant, expected] of Object.entries(expectedVariants)) {
      process.env.APP_VARIANT = appVariant;
      const config = configureMobileApp({ config: baseConfig });
      const widgetsPlugin = config.plugins.find(
        (plugin) => Array.isArray(plugin) && plugin[0] === "expo-widgets",
      );
      const devClientPlugin = config.plugins.find(
        (plugin) => Array.isArray(plugin) && plugin[0] === "expo-dev-client",
      );

      assert.equal(config.name, expected.name);
      assert.equal(config.icon, expected.icon);
      assert.equal(config.scheme, expected.scheme);
      assert.equal(config.ios.bundleIdentifier, expected.bundleIdentifier);
      assert.equal(config.ios.icon, expected.icon);
      assert.equal(config.android.package, expected.bundleIdentifier);
      assert.equal(config.android.icon, expected.icon);
      assert.equal(config.android.adaptiveIcon, undefined);
      assert.deepEqual(widgetsPlugin, [
        "expo-widgets",
        {
          bundleIdentifier: `${expected.bundleIdentifier}.widgets`,
          groupIdentifier: `group.${expected.bundleIdentifier}`,
        },
      ]);
      assert.deepEqual(devClientPlugin, [
        "expo-dev-client",
        { addGeneratedScheme: appVariant === "dev" },
      ]);
      assert.equal(config.extra.appVariant, appVariant);
      assert.equal(config.extra.appScheme, expected.scheme);
    }
  } finally {
    if (originalVariant === undefined) {
      delete process.env.APP_VARIANT;
    } else {
      process.env.APP_VARIANT = originalVariant;
    }
  }
});

test("defaults local builds to dev and rejects unknown variants", () => {
  const originalVariant = process.env.APP_VARIANT;
  try {
    delete process.env.APP_VARIANT;
    assert.equal(resolveAppVariant(), "dev");
    assert.throws(() => resolveAppVariant("preview"), /Invalid APP_VARIANT/);
  } finally {
    if (originalVariant === undefined) {
      delete process.env.APP_VARIANT;
    } else {
      process.env.APP_VARIANT = originalVariant;
    }
  }
});

test("defines dev, staging, and stable EAS build profiles", () => {
  const easConfig = JSON.parse(
    readFileSync(new URL("./eas.json", import.meta.url), "utf8"),
  );

  assert.deepEqual(Object.keys(easConfig.build), ["dev", "staging", "stable"]);
  for (const appVariant of Object.keys(expectedVariants)) {
    assert.equal(easConfig.build[appVariant].env.APP_VARIANT, appVariant);
  }
  assert.deepEqual(Object.keys(easConfig.submit), ["stable"]);
});
