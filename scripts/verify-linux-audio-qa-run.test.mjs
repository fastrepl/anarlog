import assert from "node:assert/strict";
import test from "node:test";

import { verifyLinuxAudioQaRun } from "./verify-linux-audio-qa-run.mjs";

const repository = "fastrepl/anarlog";
const version = "1.4.0";
const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const runId = "67890";

function fixture() {
  const run = {
    id: Number(runId),
    status: "completed",
    conclusion: "success",
    run_attempt: 1,
    event: "workflow_dispatch",
    head_branch: "main",
    head_sha: candidateSha,
    path: ".github/workflows/desktop_linux_audio_qa.yaml@refs/heads/main",
    repository: { id: 10, full_name: repository },
    head_repository: { id: 10, full_name: repository },
  };
  const artifacts = {
    total_count: 2,
    artifacts: ["x64", "arm64"].map((architecture, index) => ({
      id: 100 + index,
      name: `linux-audio-qa-${version}-${architecture}`,
      size_in_bytes: 1024,
      expired: false,
      digest: `sha256:${index === 0 ? "a" : "b"}`.padEnd(
        71,
        index === 0 ? "a" : "b",
      ),
      workflow_run: {
        id: Number(runId),
        repository_id: 10,
        head_repository_id: 10,
        head_branch: "main",
        head_sha: candidateSha,
      },
    })),
  };
  return { run, artifacts };
}

function verify(values) {
  return verifyLinuxAudioQaRun({
    ...values,
    repository,
    version,
    candidateSha,
    runId,
  });
}

test("accepts one exact x64 and arm64 artifact from the first run attempt", () => {
  const result = verify(fixture());
  assert.deepEqual(
    result.map((artifact) => artifact.architecture),
    ["x64", "arm64"],
  );
});

test("rejects a successful rerun with the same run ID", () => {
  const values = fixture();
  values.run.run_attempt = 2;
  assert.throws(() => verify(values), /reruns are not accepted/);
});

test("rejects missing, duplicate, expired, or unbound artifacts", () => {
  const missing = fixture();
  missing.artifacts.artifacts.pop();
  missing.artifacts.total_count = 1;
  assert.throws(() => verify(missing), /exactly two artifacts/);

  const duplicate = fixture();
  duplicate.artifacts.artifacts[1].name = duplicate.artifacts.artifacts[0].name;
  assert.throws(() => verify(duplicate), /Expected one artifact named/);

  const expired = fixture();
  expired.artifacts.artifacts[1].expired = true;
  assert.throws(() => verify(expired), /expired or empty/);

  const unbound = fixture();
  unbound.artifacts.artifacts[0].workflow_run.head_sha = "f".repeat(40);
  assert.throws(() => verify(unbound), /not bound to the expected run/);
});
