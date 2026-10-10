import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import configureMobileApp, { resolveAppVariant } from "./app.config.ts";

const baseConfig = JSON.parse(
  readFileSync(new URL("./app.json", import.meta.url), "utf8"),
).expo;
const releaseVersion = JSON.parse(
  readFileSync(new URL("./release-version.json", import.meta.url), "utf8"),
).version;

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
      assert.equal(config.version, releaseVersion);
      assert.equal(config.icon, expected.icon);
      assert.equal(config.scheme, expected.scheme);
      assert.equal(config.ios.bundleIdentifier, expected.bundleIdentifier);
      assert.equal(config.ios.icon, expected.icon);
      assert.equal(
        config.ios.infoPlist.BGTaskSchedulerPermittedIdentifiers,
        undefined,
      );
      assert.equal(config.android.package, expected.bundleIdentifier);
      assert.equal(config.android.icon, expected.icon);
      const { foregroundImage, backgroundColor, monochromeImage } =
        config.android.adaptiveIcon;
      assert.equal(
        backgroundColor,
        { dev: "#0099FF", staging: "#D1D1D1", stable: "#F7E7B0" }[appVariant],
      );
      for (const asset of [foregroundImage, monochromeImage]) {
        const png = readFileSync(new URL(asset, import.meta.url));
        assert.equal(png.toString("hex", 0, 8), "89504e470d0a1a0a");
        assert.equal(png.readUInt32BE(16), 1024);
        assert.equal(png.readUInt32BE(20), 1024);
        assert.equal(png[25], 6);
      }
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
  assert.equal(easConfig.cli.appVersionSource, "remote");
  assert.equal(easConfig.build.stable.autoIncrement, true);
  assert.equal(easConfig.build.staging.autoIncrement, true);
});

test("the mobile marketing version leaves store build counters intact", () => {
  const config = configureMobileApp({
    config: {
      ...baseConfig,
      version: "0.1.1",
      ios: { ...baseConfig.ios, version: "0.1.1", buildNumber: "42" },
      android: { ...baseConfig.android, version: "0.1.0", versionCode: 37 },
    },
  });
  assert.equal(config.version, releaseVersion);
  assert.equal(config.ios.version, releaseVersion);
  assert.equal(config.android.version, releaseVersion);
  assert.equal(config.ios.buildNumber, "42");
  assert.equal(config.android.versionCode, 37);
});
