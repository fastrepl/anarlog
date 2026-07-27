import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const WORKFLOW = ".github/workflows/desktop_linux_audio_qa.yaml";
const ARCHITECTURES = ["x64", "arm64"];

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    invariant(key?.startsWith("--") && value, `Invalid argument: ${key ?? ""}`);
    args[key.slice(2)] = value;
  }
  return args;
}

function normalizeRunId(value) {
  invariant(/^[1-9][0-9]*$/.test(String(value)), "Invalid audio QA run ID");
  const runId = Number(value);
  invariant(Number.isSafeInteger(runId), "Audio QA run ID is too large");
  return runId;
}

function normalizeSha(value) {
  invariant(
    typeof value === "string" && /^[0-9a-fA-F]{40}$/.test(value),
    "Candidate SHA must contain 40 hexadecimal characters",
  );
  return value.toLowerCase();
}

function normalizeVersion(value) {
  invariant(
    typeof value === "string" && /^[0-9]+\.[0-9]+\.[0-9]+$/.test(value),
    "Version must be a stable semantic version",
  );
  return value;
}

export function verifyLinuxAudioQaRun({
  run,
  artifacts,
  repository,
  version,
  candidateSha,
  runId,
}) {
  const expected = {
    repository,
    version: normalizeVersion(version),
    candidateSha: normalizeSha(candidateSha),
    runId: normalizeRunId(runId),
  };
  invariant(
    typeof expected.repository === "string" &&
      /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(expected.repository),
    "Invalid repository",
  );

  invariant(run?.id === expected.runId, "Audio QA run ID mismatch");
  invariant(
    run.status === "completed" && run.conclusion === "success",
    "Audio QA run did not complete successfully",
  );
  invariant(run.run_attempt === 1, "Audio QA reruns are not accepted");
  invariant(run.event === "workflow_dispatch", "Unexpected audio QA run event");
  invariant(run.head_branch === "main", "Audio QA run did not use main");
  invariant(
    run.head_sha?.toLowerCase() === expected.candidateSha,
    "Audio QA candidate SHA mismatch",
  );
  invariant(
    run.repository?.full_name === expected.repository &&
      run.head_repository?.full_name === expected.repository,
    "Audio QA repository mismatch",
  );
  invariant(
    run.path?.split("@")[0] === WORKFLOW,
    "Unexpected audio QA workflow",
  );
  invariant(
    Number.isSafeInteger(run.repository?.id) &&
      run.repository.id > 0 &&
      Number.isSafeInteger(run.head_repository?.id) &&
      run.head_repository.id > 0,
    "Audio QA repository identity is invalid",
  );

  invariant(
    artifacts?.total_count === ARCHITECTURES.length &&
      Array.isArray(artifacts.artifacts) &&
      artifacts.artifacts.length === ARCHITECTURES.length,
    "Audio QA run must contain exactly two artifacts",
  );

  return ARCHITECTURES.map((architecture) => {
    const name = `linux-audio-qa-${expected.version}-${architecture}`;
    const matches = artifacts.artifacts.filter(
      (artifact) => artifact?.name === name,
    );
    invariant(matches.length === 1, `Expected one artifact named ${name}`);
    const artifact = matches[0];
    invariant(
      Number.isSafeInteger(artifact.id) && artifact.id > 0,
      `${name} has an invalid artifact ID`,
    );
    invariant(
      artifact.expired === false &&
        Number.isSafeInteger(artifact.size_in_bytes) &&
        artifact.size_in_bytes > 0,
      `${name} is expired or empty`,
    );
    invariant(
      typeof artifact.digest === "string" &&
        /^sha256:[0-9a-f]{64}$/.test(artifact.digest),
      `${name} has no valid artifact digest`,
    );
    invariant(
      artifact.workflow_run?.id === expected.runId &&
        artifact.workflow_run?.repository_id === run.repository.id &&
        artifact.workflow_run?.head_repository_id === run.head_repository.id &&
        artifact.workflow_run?.head_branch === "main" &&
        artifact.workflow_run?.head_sha?.toLowerCase() ===
          expected.candidateSha,
      `${name} is not bound to the expected run and candidate`,
    );
    return {
      architecture,
      id: artifact.id,
      name,
      digest: artifact.digest,
    };
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  invariant(args.output, "Missing --output");
  const result = verifyLinuxAudioQaRun({
    run: JSON.parse(await readFile(args.run, "utf8")),
    artifacts: JSON.parse(await readFile(args.artifacts, "utf8")),
    repository: args.repository,
    version: args.version,
    candidateSha: args["candidate-sha"],
    runId: args["run-id"],
  });
  await writeFile(args.output, `${JSON.stringify(result, null, 2)}\n`);
  console.log("Verified exact x64 and arm64 Linux audio QA artifacts");
}

const isMain = process.argv[1]
  ? import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
  : false;

if (isMain) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
