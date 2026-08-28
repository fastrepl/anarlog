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
const CLOUD_PLUGIN_ROOT = "agent-plugins/anarlog-cloud";
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
  for (const manifestPath of PLUGIN_MANIFESTS) {
    const manifest = await readJson(manifestPath);
    assert.equal(manifest.name, "anarlog");
    assert.equal(manifest.version, "1.0.0");
    assert.equal(manifest.license, "MIT");
  }

  const portableManifest = await readJson(`${PLUGIN_ROOT}/plugin.json`);
  assert.equal(
    portableManifest.$schema,
    "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  );
});

test("repository marketplaces resolve the Anarlog plugin packages", async () => {
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
      ["anarlog", "anarlog-cloud"],
    );
    assert.equal(marketplace.plugins[0].source, `./${PLUGIN_ROOT}`);
    assert.equal(marketplace.plugins[1].source, `./${CLOUD_PLUGIN_ROOT}`);
  }

  const codexMarketplace = await readJson(".agents/plugins/marketplace.json");
  assert.equal(codexMarketplace.name, "fastrepl");
  assert.deepEqual(
    codexMarketplace.plugins.map((plugin) => plugin.name),
    ["anarlog", "anarlog-cloud"],
  );
  assert.equal(codexMarketplace.plugins[0].source.path, `./${PLUGIN_ROOT}`);
  assert.equal(
    codexMarketplace.plugins[1].source.path,
    `./${CLOUD_PLUGIN_ROOT}`,
  );
  assert.equal(codexMarketplace.plugins[1].policy.authentication, "ON_USE");
});

test("every Anarlog MCP configuration starts the local stdio server", async () => {
  const portable = await readJson(`${PLUGIN_ROOT}/mcp.json`);
  const native = await readJson(`${PLUGIN_ROOT}/.mcp.json`);
  const cursor = await readJson(`${PLUGIN_ROOT}/.cursor-plugin/plugin.json`);
  const servers = [
    portable.mcpServers.anarlog,
    native.mcpServers.anarlog,
    cursor.mcpServers.anarlog,
  ];

  for (const server of servers) {
    assert.equal(server.command, "anarlog");
    assert.deepEqual(server.args, ["mcp"]);
  }
  assert.equal(portable.mcpServers.anarlog.type, "stdio");

  const claude = await readJson(`${PLUGIN_ROOT}/.claude-plugin/plugin.json`);
  assert.equal(claude.mcpServers, "./.mcp.json");

  const codexManifest = await readJson(
    `${PLUGIN_ROOT}/.codex-plugin/plugin.json`,
  );
  assert.equal(codexManifest.mcpServers, "./.mcp.json");
});

test("Anarlog Cloud is a distinct MCP-only OAuth plugin across hosts", async () => {
  const mcp = await readJson(`${CLOUD_PLUGIN_ROOT}/.mcp.json`);
  const portableMcp = await readJson(`${CLOUD_PLUGIN_ROOT}/mcp.json`);
  const expectedServer = {
    type: "http",
    url: "https://api.anarlog.so/mcp",
  };

  for (const manifestPath of [
    `${CLOUD_PLUGIN_ROOT}/plugin.json`,
    `${CLOUD_PLUGIN_ROOT}/.claude-plugin/plugin.json`,
    `${CLOUD_PLUGIN_ROOT}/.codex-plugin/plugin.json`,
    `${CLOUD_PLUGIN_ROOT}/.cursor-plugin/plugin.json`,
  ]) {
    const manifest = await readJson(manifestPath);
    assert.equal(manifest.name, "anarlog-cloud");
    assert.equal(manifest.version, "1.0.0");
    assert.equal(manifest.license, "MIT");
    assert.equal(manifest.skills, undefined);
  }

  const portable = await readJson(`${CLOUD_PLUGIN_ROOT}/plugin.json`);
  assert.equal(
    portable.$schema,
    "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  );

  const claude = await readJson(
    `${CLOUD_PLUGIN_ROOT}/.claude-plugin/plugin.json`,
  );
  const codex = await readJson(
    `${CLOUD_PLUGIN_ROOT}/.codex-plugin/plugin.json`,
  );
  assert.equal(claude.mcpServers, "./.mcp.json");
  assert.equal(codex.mcpServers, "./.mcp.json");

  const cursor = await readJson(
    `${CLOUD_PLUGIN_ROOT}/.cursor-plugin/plugin.json`,
  );
  assert.deepEqual(cursor.mcpServers["anarlog-cloud"], {
    url: "https://api.anarlog.so/mcp",
  });

  assert.deepEqual(mcp.mcpServers["anarlog-cloud"], expectedServer);
  assert.deepEqual(portableMcp.mcpServers["anarlog-cloud"], expectedServer);
});

test("marketplace icon is a 400px square PNG", async () => {
  for (const iconPath of [
    `${PLUGIN_ROOT}/assets/icon.png`,
    `${CLOUD_PLUGIN_ROOT}/assets/icon.png`,
  ]) {
    const icon = await readFile(iconPath);
    assert.equal(icon.subarray(1, 4).toString("ascii"), "PNG");
    assert.equal(icon.readUInt32BE(16), 400);
    assert.equal(icon.readUInt32BE(20), 400);
    assert.ok(icon.length < 5 * 1024 * 1024);
  }
});

test("the transformation changes nothing but the defined reference links", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  let restored = publishSkill(canonical);
  for (const [reference, url] of Object.entries(REFERENCE_LINK_REWRITES)) {
    restored = restored.split(`](${url})`).join(`](${reference})`);
  }

  assert.equal(restored, canonical);
});

test("non-link drift in the published mirror is detected", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  const published = await readFile(PUBLISHED_SKILL_PATH, "utf8");
  const sentinel = "staging proposals";

  assert.match(canonical, new RegExp(sentinel));
  assert.notEqual(
    publishSkill(canonical.replace(sentinel, "direct writes")),
    published,
  );
});

test("reference links without a public mapping fail publishing", () => {
  assert.throws(
    () => publishSkill("see [new page](references/new-page.md)"),
    /no public URL mapping/,
  );
});
