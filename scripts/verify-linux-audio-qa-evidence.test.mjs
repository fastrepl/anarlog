import assert from "node:assert/strict";
import { mkdir, mkdtemp, rename, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { verifyLinuxAudioQaEvidence } from "./verify-linux-audio-qa-evidence.mjs";

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
const thresholds = {
  duration_delta_seconds_max: 0.15,
  mic_active_rms_min: 0.0002,
  speaker_active_rms_min: 0.001,
  system_tone_amplitude_min: 0.0005,
  mic_pilot_amplitude_min: 0.0002,
  mic_isolation_db_min: 12,
  speaker_isolation_db_min: 15,
  system_tone_isolation_db_min: 18,
  mic_pilot_isolation_db_min: 6,
  cross_channel_isolation_db_min: 12,
  clipped_fraction_max: 0.001,
  recording_tail_tolerance_seconds: 0.5,
};
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
      JSON.stringify({
        status: "pass",
        scope:
          "Virtual microphone/system routing, capture, separation, and persistence with NO_AEC=1. This result makes no AEC claim.",
        failures: [],
        thresholds,
        metrics: Object.fromEntries(metricNames.map((name) => [name, 1])),
      }),
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

test("accepts exact passing x64 and arm64 evidence", async () => {
  const fixture = await createFixture();
  const results = await verifyLinuxAudioQaEvidence({
    ...fixture,
    version,
    candidateSha,
    dryRunId,
    audioQaRunId,
  });

  assert.deepEqual(
    results.map((result) => result.artifactArch),
    ["x64", "arm64"],
  );
});

test("rejects failed or incomplete architecture evidence", async () => {
  const fixture = await createFixture();
  const analysisPath = path.join(
    fixture.evidenceDir,
    "arm64",
    "qa-artifacts",
    "linux-arm64",
    "analysis.json",
  );
  await writeFile(
    analysisPath,
    JSON.stringify({ status: "fail", failures: ["leakage"], metrics: {} }),
  );

  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fixture,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /arm64 audio analysis did not pass/,
  );
});

test("rejects evidence from a different run or package hash", async () => {
  const fixture = await createFixture();

  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fixture,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId: "99999",
    }),
    /x64 audio QA run mismatch/,
  );

  fixture.manifest.assets[0].sha256 = "c".repeat(64);
  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fixture,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /x64 asset does not match release provenance/,
  );
});

test("rejects a basename decoy outside the exact evidence path", async () => {
  const fixture = await createFixture();
  const exact = path.join(
    fixture.evidenceDir,
    "x64",
    "qa-artifacts",
    "linux-x64",
    "provenance.json",
  );
  const decoyDirectory = path.join(
    fixture.evidenceDir,
    "x64",
    "e2e",
    "blackbox",
    "videos",
  );
  await mkdir(decoyDirectory, { recursive: true });
  await rename(exact, path.join(decoyDirectory, "provenance.json"));

  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fixture,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /x64 evidence is missing .*provenance\.json/,
  );
});

test("rejects incomplete analysis and invalid phase scope", async () => {
  const fixture = await createFixture();
  const root = path.join(
    fixture.evidenceDir,
    "x64",
    "qa-artifacts",
    "linux-x64",
  );
  await writeFile(
    path.join(root, "analysis.json"),
    JSON.stringify({
      status: "pass",
      scope:
        "Virtual microphone/system routing, capture, separation, and persistence with NO_AEC=1. This result makes no AEC claim.",
      failures: [],
      thresholds,
      metrics: { duration: 30 },
    }),
  );

  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fixture,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /x64 audio analysis did not pass/,
  );

  const refreshed = await createFixture();
  const phasesPath = path.join(
    refreshed.evidenceDir,
    "arm64",
    "qa-artifacts",
    "linux-arm64",
    "phases.json",
  );
  await writeFile(
    phasesPath,
    JSON.stringify({
      schema_version: 1,
      no_aec: false,
      recording_stop_seconds: 35,
      phases: [
        { name: "mic_only", start_seconds: 1, end_seconds: 7 },
        { name: "system_only", start_seconds: 8, end_seconds: 14 },
        { name: "both", start_seconds: 15, end_seconds: 21 },
      ],
    }),
  );
  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...refreshed,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /arm64 phase evidence is invalid/,
  );
});

test("rejects the wrong package, checksum, or capture backend", async () => {
  const wrongPackage = await createFixture();
  const x64Root = path.join(
    wrongPackage.evidenceDir,
    "x64",
    "qa-artifacts",
    "linux-x64",
  );
  await writeFile(
    path.join(x64Root, "debian-package.txt"),
    `package=other\nversion=${version}\narchitecture=amd64\nsource=/candidate.deb\n`,
  );
  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...wrongPackage,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /x64 Debian package identity mismatch/,
  );

  const wrongChecksum = await createFixture();
  await writeFile(
    path.join(
      wrongChecksum.evidenceDir,
      "arm64",
      "qa-artifacts",
      "linux-arm64",
      "candidate.deb.sha256",
    ),
    `${"f".repeat(64)}  candidate.deb\n`,
  );
  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...wrongChecksum,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /arm64 tested package hash mismatch/,
  );

  const fallback = await createFixture();
  await writeFile(
    path.join(
      fallback.evidenceDir,
      "x64",
      "qa-artifacts",
      "linux-x64",
      "capture-backend-events.txt",
    ),
    "pipewire_capture_initialized\npulseaudio_capture_initialized\n",
  );
  await assert.rejects(
    verifyLinuxAudioQaEvidence({
      ...fallback,
      version,
      candidateSha,
      dryRunId,
      audioQaRunId,
    }),
    /x64 did not prove direct PipeWire capture/,
  );
});
