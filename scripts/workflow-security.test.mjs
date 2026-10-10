import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

test("staging and release result steps treat shell metacharacters as literal data", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "workflow-security-"));
  const hostile = "fix/$(touch${IFS}injected)`touch${IFS}injected`";
  try {
    for (const workflow of ["handle_staging.yaml", "handle_release.yaml"]) {
      const source = await readFile(
        new URL(`../.github/workflows/${workflow}`, import.meta.url),
        "utf8",
      );
      const step = source
        .split("      - id: result\n")[1]
        .split("      - uses:")[0];
      const [configuration, run] = step.split("        run: |\n");
      const values = {
        "inputs.channel": hostile,
        "steps.resolve.outputs.branch": hostile,
        "steps.resolve.outputs.sha": "0123456789abcdef",
        "steps.resolve.outputs.target_sha": "0123456789abcdef",
        "steps.resolve.outputs.resolved": "true",
        "steps.resolve.outputs.error": "",
        "steps.trigger.outputs.run_id": "123",
        "github.run_id": "456",
      };
      const interpolate = (text) =>
        text.replace(/\$\{\{\s*(.*?)\s*\}\}/g, (_, expression) => {
          assert.ok(
            expression in values,
            `Unknown fixture expression ${expression}`,
          );
          return values[expression];
        });
      const env = {
        PATH: process.env.PATH,
        REPO: "example/repository",
        GITHUB_OUTPUT: path.join(directory, workflow),
      };
      for (const line of configuration.split("\n")) {
        const assignment = line.match(/^          ([A-Z_]+): (.*)$/);
        if (assignment) env[assignment[1]] = interpolate(assignment[2]);
      }
      const result = spawnSync(
        "bash",
        ["-e", "-c", interpolate(run.replace(/^          /gm, ""))],
        { cwd: directory, env, encoding: "utf8" },
      );
      assert.equal(result.status, 0, result.stderr);
      const output = await readFile(env.GITHUB_OUTPUT, "utf8");
      const data = JSON.parse(output.trim().slice("data=".length));
      assert.ok(data[0].value.includes(hostile));
      await assert.rejects(readFile(path.join(directory, "injected")), {
        code: "ENOENT",
      });
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
