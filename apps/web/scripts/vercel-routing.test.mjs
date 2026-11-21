import assert from "node:assert/strict";
import test from "node:test";

import {
  desktopUpdateRoutes,
  GPUI_BRIDGE_VERSION,
  vercelBuildConfig,
  versionsFrom,
} from "../vercel-build-config.ts";

function matches(condition, request) {
  if (condition.type === "host") return condition.value === request.hostname;
  if (condition.type === "query") {
    return request.searchParams.get(condition.key) === condition.value;
  }
  return false;
}

// Vercel appends the request's query to a redirect's Location (desktop.anarlog.so does today).
function routeFor(url, routes = vercelBuildConfig.routes) {
  const request = new URL(url);
  for (const route of routes) {
    if (route.continue) continue;
    if (route.has?.some((condition) => !matches(condition, request))) continue;
    const match = new RegExp(route.src).exec(request.pathname);
    if (!match) continue;
    const destination = (route.dest ?? route.headers?.Location)?.replace(
      /\$(\d+)/g,
      (_, index) => match[Number(index)],
    );
    const redirect = route.status >= 300 && route.status < 400;
    return {
      ...route,
      destination: redirect ? `${destination}${request.search}` : destination,
    };
  }
}

const TAURI_FEED = "https://cdn.crabnebula.app/update/fastrepl/hyprnote2";
const GPUI_FEED = "https://cdn.crabnebula.app/update/fastrepl/anarlog-gpui";
const PLATFORMS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-x86_64-appimage",
  "linux-aarch64-deb",
  "windows-x86_64-nsis",
];

function compare(left, right) {
  for (let index = 0; index < 3; index++) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

test("APT redirects retain the release and architecture", () => {
  for (const [arch, asset] of [
    ["amd64", "x86_64"],
    ["arm64", "aarch64"],
  ]) {
    const route = routeFor(
      `https://anarlog.so/apt/pool/desktop_v1.4.22/${arch}/anarlog.deb`,
    );
    assert.equal(route.status, 302);
    assert.equal(
      route.destination,
      `https://github.com/fastrepl/anarlog/releases/download/desktop_v1.4.22/anarlog-linux-${asset}.deb`,
    );
  }
});

test("the feed gate sends the bridge and later stable clients to the GPUI feed", () => {
  const routes = desktopUpdateRoutes("1.4.36");
  for (const platform of PLATFORMS) {
    for (const version of [
      "1.4.36",
      "1.4.37",
      "1.4.100",
      "1.5.0",
      "1.10.0",
      "2.0.0",
    ]) {
      const path = `${platform}/${version}?channel=stable`;
      assert.equal(
        routeFor(`https://desktop.anarlog.so/update/${path}`, routes)
          ?.destination,
        `${GPUI_FEED}/${path}`,
      );
    }
    for (const path of [
      `${platform}/1.4.35?channel=stable`,
      `${platform}/1.4.4?channel=stable`,
      `${platform}/1.3.99?channel=stable`,
      `${platform}/0.9.9?channel=stable`,
      `${platform}/1.5.0-nightly.3?channel=stable`,
      `${platform}/1.5.0-dev.1%2Babc1234?channel=stable`,
      `${platform}/1.5.0?channel=nightly`,
      `${platform}/1.5.0`,
    ]) {
      assert.equal(
        routeFor(`https://desktop.anarlog.so/update/${path}`, routes)
          ?.destination,
        `${TAURI_FEED}/${path}`,
        path,
      );
    }
  }
  assert.equal(
    routeFor(
      "https://anarlog.so/update/darwin-aarch64/1.5.0?channel=stable",
      routes,
    ),
    undefined,
  );
});

test("the gate's version pattern is semver order from the bridge", () => {
  for (const bridge of [
    "1.4.36",
    "0.0.0",
    "1.9.9",
    "9.0.10",
    "10.10.0",
    "2.0.99",
  ]) {
    const from = new RegExp(`^(?:${versionsFrom(bridge)})$`);
    const floor = bridge.split(".").map(Number);
    for (let major = 0; major <= 12; major++) {
      for (let minor = 0; minor <= 12; minor++) {
        for (let patch = 0; patch <= 120; patch++) {
          const version = [major, minor, patch];
          assert.equal(
            from.test(version.join(".")),
            compare(version, floor) >= 0,
            `${version.join(".")} from ${bridge}`,
          );
        }
      }
    }
  }
  assert.throws(() => versionsFrom("1.4"));
  assert.throws(() => versionsFrom("1.4.36-nightly.1"));
});

test("the deployed feed gate opens at its bridge version", () => {
  const path = (version) => `darwin-aarch64/${version}?channel=stable`;
  const feed = GPUI_BRIDGE_VERSION === null ? TAURI_FEED : GPUI_FEED;
  for (const version of [GPUI_BRIDGE_VERSION ?? "1.4.36", "99.0.0"]) {
    assert.equal(
      routeFor(`https://desktop.anarlog.so/update/${path(version)}`)
        ?.destination,
      `${feed}/${path(version)}`,
    );
  }
});

test("desktop updater redirects are restricted to the desktop hostname", () => {
  assert.equal(
    routeFor("https://desktop.anarlog.so/update/darwin/aarch64/1.4.22")
      ?.destination,
    "https://cdn.crabnebula.app/update/fastrepl/hyprnote2/darwin/aarch64/1.4.22",
  );
  assert.equal(
    routeFor("https://anarlog.so/update/darwin/aarch64/1.4.22"),
    undefined,
  );
});

test("agent discovery proxies retain paths and their required origin header", () => {
  for (const path of [
    "/skill.md",
    "/.well-known/skills/anarlog/SKILL.md",
    "/.well-known/agent-skills/index.json",
    "/.well-known/vercel/flags",
  ]) {
    const route = routeFor(`https://anarlog.so${path}`);
    assert.equal(route.destination, `https://docs.anarlog.so${path}`);
    assert.deepEqual(route.transforms, [
      {
        type: "request.headers",
        op: "set",
        target: { key: "Origin" },
        args: "docs.anarlog.so",
      },
    ]);
  }
});

test("public asset redirects and legacy host redirects preserve suffixes", () => {
  for (const path of ["blog/library/image.webp", "team/john.png"]) {
    const route = routeFor(`https://anarlog.so/api/assets/${path}`);
    assert.equal(route?.status, 301);
    assert.equal(route?.destination, `https://static.anarlog.so/${path}`);
  }
  assert.equal(
    routeFor("https://char.com/blog/old-article/")?.destination,
    "https://anarlog.so/blog/old-article/",
  );
  assert.equal(routeFor("https://anarlog.so/blog/old-article/"), undefined);
});

// Canonical navigation uses trailingSlash: "always". CDN redirects run first.
test("exact page redirects accept canonical trailing slashes", () => {
  for (const [host, path, destination] of [
    ["anarlog.so", "/faq", "/"],
    ["anarlog.so", "/roadmap", "/changelog/"],
    ["anarlog.so", "/skill", "/skill.md"],
    ["anarlog.so", "/skills", "https://docs.anarlog.so/agents/skills"],
    ["anarlog.so", "/docs", "https://docs.anarlog.so"],
    ["anarlog.so", "/blog/filesystem-is-coretex", "/blog/"],
    ["hyprnote.com", "/auth", "https://char.com/auth"],
    ["hyprnote.com", "/download", "https://char.com/download"],
    ["hyprnote.com", "/pricing", "https://char.com/pricing"],
    ["char.com", "/privacy", "https://anarlog.so/privacy"],
    ["www.char.com", "/terms", "https://anarlog.so/terms"],
  ]) {
    for (const suffix of ["", "/"]) {
      const url = `https://${host}${path}${suffix}`;
      const route = routeFor(url);
      assert.equal(route?.status, 301, url);
      assert.equal(route?.destination, destination, url);
    }
  }
});

test("the About page is reachable on Anarlog and legacy hosts keep their redirect", () => {
  for (const suffix of ["", "/"]) {
    assert.equal(routeFor(`https://anarlog.so/about${suffix}`), undefined);
    assert.equal(
      routeFor(`https://hyprnote.com/about${suffix}`)?.destination,
      "https://char.com/about",
    );
  }
});
