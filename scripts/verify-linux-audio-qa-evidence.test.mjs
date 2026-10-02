import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, rename, writeFile, readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

import {
  linuxAudioQaPolicy,
  verifyLinuxAudioQaEvidence,
} from "./verify-linux-audio-qa-evidence.mjs";

const version = "1.4.0";
const candidateSha = "0123456789abcdef0123456789abcdef01234567";
const dryRunId = "12345";
const audioQaRunId = "67890";
const hashes = {
  x64: "a".repeat(64),
  arm64: "b".repeat(64),
};
const architectures = {
  x64: {
    publicPlatform: "debian-x86_64",
    updatePlatform: "linux-x86_64-deb",
    debianArch: "amd64",
    assetId: "asset-x64",
  },
  arm64: {
    publicPlatform: "debian-aarch64",
    updatePlatform: "linux-aarch64-deb",
    debianArch: "arm64",
    assetId: "asset-arm64",
  },
};
const thresholds = linuxAudioQaPolicy.thresholds;
const metricNames = [
  "mic_duration_seconds",
  "speaker_duration_seconds",
  "duration_delta_seconds",
  "alignment_offset_seconds",
  "recording_stop_seconds",
  "aligned_recording_stop_seconds",
  "mic_recording_tail_seconds",
  "speaker_recording_tail_seconds",
  "mic_clipped_fraction",
  "speaker_clipped_fraction",
  "mic_only_mic_rms",
  "system_only_mic_rms",
  "both_mic_rms",
  "mic_only_speaker_rms",
  "system_only_speaker_rms",
  "both_speaker_rms",
  "mic_only_523hz_amplitude",
  "system_only_523hz_amplitude",
  "both_523hz_amplitude",
  "mic_only_997hz_amplitude",
  "system_only_997hz_amplitude",
  "both_997hz_amplitude",
  "both_mic_997hz_amplitude",
  "both_speaker_523hz_amplitude",
  "mic_only_isolation_db",
  "both_mic_isolation_db",
  "system_only_speaker_isolation_db",
  "both_speaker_isolation_db",
  "system_tone_isolation_db",
  "mic_pilot_isolation_db",
  "both_system_to_mic_isolation_db",
  "both_mic_to_speaker_isolation_db",
];

const passingAnalysis = {
  status: "pass",
  scope:
    "Virtual microphone/system routing, capture, separation, and persistence with NO_AEC=1. This result makes no AEC claim.",
  failures: [],
  thresholds,
  metrics: Object.fromEntries(metricNames.map((name) => [name, 1])),
};

function wavFixture() {
  const buffer = Buffer.alloc(45);
  buffer.write("RIFF", 0, "ascii");
  buffer.writeUInt32LE(37, 4);
  buffer.write("WAVE", 8, "ascii");
  return buffer;
}

async function createFixture() {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-linux-audio-evidence-"),
  );
  const evidenceDir = path.join(directory, "evidence");
  const manifest = {
    schemaVersion: 1,
    sourceWorkflow: ".github/workflows/desktop_cd.yaml",
    workflowRunId: dryRunId,
    candidateSha,
    version,
    channel: "stable",
    publish: false,
    assets: Object.entries(architectures).map(
      ([artifactArch, architecture]) => ({
        id: architecture.assetId,
        publicPlatform: architecture.publicPlatform,
        updatePlatform: architecture.updatePlatform,
        size: 1024,
        signature: "signature",
        sha256: hashes[artifactArch],
      }),
    ),
  };

  for (const [artifactArch, architecture] of Object.entries(architectures)) {
    const root = path.join(
      evidenceDir,
      artifactArch,
      "qa-artifacts",
      `linux-${artifactArch}`,
    );
    await mkdir(root, { recursive: true });
    await writeFile(
      path.join(root, "provenance.json"),
      JSON.stringify({
        application: "fastrepl/hyprnote2",
        version,
        candidate_sha: candidateSha,
        workflow_run_id: audioQaRunId,
        source_workflow_run_id: dryRunId,
        public_platform: architecture.publicPlatform,
        crabnebula_asset_id: architecture.assetId,
        crabnebula_asset_size: 1024,
        expected_sha256: hashes[artifactArch],
      }),
    );
    await writeFile(
      path.join(root, "crabnebula-asset.json"),
      JSON.stringify({
        id: architecture.assetId,
        filename: `anarlog-${artifactArch}.deb`,
        publicPlatform: architecture.publicPlatform,
        updatePlatform: architecture.updatePlatform,
        size: 1024,
      }),
    );
    await writeFile(
      path.join(root, "download.sha256"),
      `${hashes[artifactArch]}  candidate-linux-${artifactArch}.deb\n`,
    );
    await writeFile(
      path.join(root, "candidate.deb.sha256"),
      `${hashes[artifactArch]}  candidate.deb\n`,
    );
    await writeFile(
      path.join(root, "debian-package.txt"),
      `package=anarlog\nversion=${version}\narchitecture=${architecture.debianArch}\nsource=/candidate.deb\n`,
    );
    await writeFile(
      path.join(root, "analysis.json"),
      JSON.stringify(passingAnalysis),
    );
    await writeFile(
      path.join(root, "phases.json"),
      JSON.stringify({
        schema_version: 1,
        no_aec: true,
        recording_stop_seconds: 35,
        phases: [
          { name: "mic_only", start_seconds: 1, end_seconds: 7 },
          { name: "system_only", start_seconds: 8, end_seconds: 14 },
          { name: "both", start_seconds: 15, end_seconds: 21 },
        ],
      }),
    );
    await writeFile(path.join(root, "capture-errors.txt"), "");
    await writeFile(
      path.join(root, "capture-backend-events.txt"),
      "pipewire_capture_initialized\n",
    );
    await writeFile(path.join(root, "audio_mic.wav"), wavFixture());
    await writeFile(path.join(root, "audio_spk.wav"), wavFixture());
    await mkdir(path.join(root, "release-provenance"));
    await writeFile(
      path.join(root, "release-provenance", "manifest.json"),
      JSON.stringify(manifest),
    );
  }

  return { evidenceDir, manifest };
}

function evidenceRoot(fixture, artifactArch) {
  return path.join(
    fixture.evidenceDir,
    artifactArch,
    "qa-artifacts",
    `linux-${artifactArch}`,
  );
}

function verify(fixture, overrides = {}) {
  return verifyLinuxAudioQaEvidence({
    ...fixture,
    version,
    candidateSha,
    dryRunId,
    audioQaRunId,
    ...overrides,
  });
}

test("accepts exact passing x64 and arm64 evidence", async () => {
  const results = await verify(await createFixture());

  assert.deepEqual(
    results.map((result) => result.artifactArch),
    ["x64", "arm64"],
  );
});

for (const [name, mutate, error] of [
  [
    "a failed analysis",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "arm64"), "analysis.json"),
        JSON.stringify({ status: "fail", failures: ["leakage"], metrics: {} }),
      ),
    /arm64 audio analysis did not pass/,
  ],
  [
    "incomplete analysis metrics",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "x64"), "analysis.json"),
        JSON.stringify({ ...passingAnalysis, metrics: { duration: 30 } }),
      ),
    /x64 audio analysis did not pass/,
  ],
  [
    "analysis thresholds that deviate from the shared policy",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "x64"), "analysis.json"),
        JSON.stringify({
          ...passingAnalysis,
          thresholds: {
            ...thresholds,
            mic_isolation_db_min: thresholds.mic_isolation_db_min - 1,
          },
        }),
      ),
    /x64 audio analysis did not pass/,
  ],
  [
    "phase evidence captured with AEC enabled",
    async (fixture) => {
      const file = path.join(evidenceRoot(fixture, "arm64"), "phases.json");
      const phases = JSON.parse(await readFile(file, "utf8"));
      await writeFile(file, JSON.stringify({ ...phases, no_aec: false }));
    },
    /arm64 phase evidence is invalid/,
  ],
  [
    "evidence from a different audio QA run",
    () => ({ audioQaRunId: "99999" }),
    /x64 audio QA run mismatch/,
  ],
  [
    "a package hash that differs from release provenance",
    (fixture) => {
      fixture.manifest.assets[0].sha256 = "c".repeat(64);
    },
    /x64 asset does not match release provenance/,
  ],
  [
    "a basename decoy outside the exact evidence path",
    async (fixture) => {
      const decoyDirectory = path.join(
        fixture.evidenceDir,
        "x64",
        "e2e",
        "blackbox",
        "videos",
      );
      await mkdir(decoyDirectory, { recursive: true });
      await rename(
        path.join(evidenceRoot(fixture, "x64"), "provenance.json"),
        path.join(decoyDirectory, "provenance.json"),
      );
    },
    /x64 evidence is missing .*provenance\.json/,
  ],
  [
    "the wrong Debian package",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "x64"), "debian-package.txt"),
        `package=other\nversion=${version}\narchitecture=amd64\nsource=/candidate.deb\n`,
      ),
    /x64 Debian package identity mismatch/,
  ],
  [
    "a tested package checksum mismatch",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "arm64"), "candidate.deb.sha256"),
        `${"f".repeat(64)}  candidate.deb\n`,
      ),
    /arm64 tested package hash mismatch/,
  ],
  [
    "a PulseAudio capture fallback",
    (fixture) =>
      writeFile(
        path.join(evidenceRoot(fixture, "x64"), "capture-backend-events.txt"),
        "pipewire_capture_initialized\npulseaudio_capture_initialized\n",
      ),
    /x64 did not prove direct PipeWire capture/,
  ],
]) {
  test(`rejects ${name}`, async () => {
    const fixture = await createFixture();
    const overrides = await mutate(fixture);
    await assert.rejects(verify(fixture, overrides ?? {}), error);
  });
}

test("python producer and node verifier read the same policy artifact", async () => {
  const pythonPolicy = JSON.parse(
    (
      await promisify(execFile)("python3", [
        "-c",
        [
          "import importlib.util, json",
          "spec = importlib.util.spec_from_file_location('v', 'scripts/qa/verify_linux_audio_tracks.py')",
          "module = importlib.util.module_from_spec(spec)",
          "spec.loader.exec_module(module)",
          "print(json.dumps({'thresholds': module.THRESHOLDS, 'requiredPhases': sorted(module.REQUIRED_PHASES), 'minPhaseDurationSeconds': module.MIN_PHASE_DURATION_SECONDS, 'phaseSchemaVersion': module.PHASE_SCHEMA_VERSION}))",
        ].join("\n"),
      ])
    ).stdout,
  );

  assert.deepEqual(pythonPolicy, {
    thresholds: linuxAudioQaPolicy.thresholds,
    requiredPhases: [...linuxAudioQaPolicy.requiredPhases].sort(),
    minPhaseDurationSeconds: linuxAudioQaPolicy.minPhaseDurationSeconds,
    phaseSchemaVersion: linuxAudioQaPolicy.phaseSchemaVersion,
  });
});
