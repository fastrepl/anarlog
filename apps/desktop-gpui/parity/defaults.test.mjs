import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { commandDefaults } from "./defaults.mjs";

const repository = fileURLToPath(new URL("../../..", import.meta.url));

test("commandDefaults preserves commented fields and multiline aliases", () => {
  const defaults = commandDefaults(repository);

  assert.deepEqual(defaults["plugin:camera-watermark|get_state"].automation, {
    switches_camera: [],
    removes_app_background: [],
    manual_app_background: [],
  });
  assert.deepEqual(defaults.check_embedded_cli, {
    supported: false,
    commandName: "",
    installPath: "",
    state: "installed",
    details: null,
  });
  assert.deepEqual(defaults.install_agent_skill, {
    agent: "claude_code",
    displayName: "",
    detected: false,
    installed: false,
    skillPath: "",
  });
  assert.equal(defaults.get_database_dir, "");
});
