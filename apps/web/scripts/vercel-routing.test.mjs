import assert from "node:assert/strict";
import test from "node:test";

import { vercelBuildConfig } from "../vercel-build-config.ts";

function routeFor(url) {
  const request = new URL(url);
  for (const route of vercelBuildConfig.routes) {
    if (route.continue) continue;
    if (
      route.has?.some(
        (condition) =>
          condition.type === "host" && condition.value !== request.hostname,
      )
    )
      continue;
    const match = new RegExp(route.src).exec(request.pathname);
    if (!match) continue;
    const destination = (route.dest ?? route.headers?.Location)?.replace(
      /\$(\d+)/g,
      (_, index) => match[Number(index)],
    );
    return { ...route, destination };
  }
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
  assert.equal(
    routeFor("https://anarlog.so/api/assets/blog/library/image.webp")
      ?.destination,
    "/images/blog/library/image.webp",
  );
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
