import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  CANONICAL_SKILL_PATH,
  PLUGIN_PACKAGE_MIRRORS,
  PUBLISHED_SKILL_PATH,
  publishSkill,
  REFERENCE_LINK_REWRITES,
} from "./publish-anarlog-skill.mjs";

const PLUGIN_ROOT = "agent-plugins/anarlog";
const PLUGIN_MANIFESTS = [
  `${PLUGIN_ROOT}/plugin.json`,
  `${PLUGIN_ROOT}/.claude-plugin/plugin.json`,
  `${PLUGIN_ROOT}/.codex-plugin/plugin.json`,
  `${PLUGIN_ROOT}/.cursor-plugin/plugin.json`,
];

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, "utf8"));
}

test("published skill equals the canonical skill after only link rewrites", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  const published = await readFile(PUBLISHED_SKILL_PATH, "utf8");

  assert.equal(published, publishSkill(canonical));
});

test("plugin package mirrors the canonical skill and license", async () => {
  for (const [source, target] of PLUGIN_PACKAGE_MIRRORS) {
    assert.equal(
      await readFile(target, "utf8"),
      await readFile(source, "utf8"),
    );
  }
});

test("plugin manifests use one stable identity and version", async () => {
  const [portable, ...manifests] = await Promise.all(
    PLUGIN_MANIFESTS.map(readJson),
  );
  for (const manifest of [portable, ...manifests]) {
    assert.equal(manifest.name, "anarlog");
    assert.equal(manifest.version, portable.version);
    assert.equal(manifest.license, "MIT");
  }
});

test("repository marketplaces list a single Anarlog plugin", async () => {
  const { version } = await readJson(`${PLUGIN_ROOT}/plugin.json`);
  const marketplacePaths = [
    ".claude-plugin/marketplace.json",
    ".cursor-plugin/marketplace.json",
    ".github/plugin/marketplace.json",
  ];
  for (const marketplacePath of marketplacePaths) {
    const marketplace = await readJson(marketplacePath);
    assert.equal(marketplace.name, "fastrepl");
    assert.deepEqual(
      marketplace.plugins.map((plugin) => plugin.name),
      ["anarlog"],
    );
    assert.equal(marketplace.plugins[0].source, `./${PLUGIN_ROOT}`);
    assert.equal(marketplace.plugins[0].version, version);
    assert.equal(marketplace.metadata.version, version);
  }

  const codexMarketplace = await readJson(".agents/plugins/marketplace.json");
  assert.equal(codexMarketplace.name, "fastrepl");
  assert.deepEqual(
    codexMarketplace.plugins.map((plugin) => plugin.name),
    ["anarlog"],
  );
  assert.equal(codexMarketplace.plugins[0].source.path, `./${PLUGIN_ROOT}`);
  assert.equal(codexMarketplace.plugins[0].policy.authentication, "ON_USE");
});

test("the Anarlog plugin connects Cloud MCP over HTTP", async () => {
  const portable = await readJson(`${PLUGIN_ROOT}/mcp.json`);
  const native = await readJson(`${PLUGIN_ROOT}/.mcp.json`);
  const expectedServer = {
    type: "http",
    url: "https://api.anarlog.so/mcp",
  };

  assert.deepEqual(portable.mcpServers.anarlog, {
    ...expectedServer,
    type: "streamable-http",
  });
  assert.deepEqual(native.mcpServers.anarlog, expectedServer);

  const cursor = await readJson(`${PLUGIN_ROOT}/.cursor-plugin/plugin.json`);
  assert.deepEqual(cursor.mcpServers.anarlog, {
    url: "https://api.anarlog.so/mcp",
  });

  const claude = await readJson(`${PLUGIN_ROOT}/.claude-plugin/plugin.json`);
  const codex = await readJson(`${PLUGIN_ROOT}/.codex-plugin/plugin.json`);
  assert.equal(claude.mcpServers, "./.mcp.json");
  assert.equal(codex.mcpServers, "./.mcp.json");
  assert.equal(claude.skills, "./skills/");
  assert.equal(codex.skills, "./skills/");
  assert.equal(cursor.skills, "./skills/");
});

test("the transformation changes nothing but the defined reference links", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  let restored = publishSkill(canonical);
  for (const [reference, url] of Object.entries(REFERENCE_LINK_REWRITES)) {
    restored = restored.split(`](${url})`).join(`](${reference})`);
  }

  assert.equal(restored, canonical);
});

test("reference links without a public mapping fail publishing", () => {
  assert.throws(
    () => publishSkill("see [new page](references/new-page.md)"),
    /no public URL mapping/,
  );
});
