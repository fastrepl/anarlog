import { lstat, readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const APPLICATION = "fastrepl/hyprnote2";
const SOURCE_WORKFLOW = ".github/workflows/desktop_cd.yaml";
const ARCHITECTURES = [
  {
    artifactArch: "x64",
    publicPlatform: "deb-x86_64",
    debianArch: "amd64",
  },
  {
    artifactArch: "arm64",
    publicPlatform: "deb-aarch64",
    debianArch: "arm64",
  },
];
const ANALYSIS_SCOPE =
  "Virtual microphone/system routing, capture, separation, and persistence with NO_AEC=1. This result makes no AEC claim.";
const ANALYSIS_THRESHOLDS = {
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
const ANALYSIS_METRICS = [
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

function normalizeVersion(value) {
  invariant(
    typeof value === "string" && /^[0-9]+\.[0-9]+\.[0-9]+$/.test(value),
    "Version must be a stable semantic version",
  );
  return value;
}

function normalizeSha(value) {
  invariant(
    typeof value === "string" && /^[0-9a-fA-F]{40}$/.test(value),
    "Candidate SHA must contain 40 hexadecimal characters",
  );
  return value.toLowerCase();
}

function normalizeRunId(value, label) {
  const runId = String(value);
  invariant(/^[1-9][0-9]*$/.test(runId), `${label} must be a positive integer`);
  return runId;
}

function normalizeSha256(value, label) {
  invariant(
    typeof value === "string" && /^[0-9a-f]{64}$/.test(value),
    `${label} must be a lowercase SHA-256`,
  );
  return value;
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function requireFile(root, basename, artifactArch) {
  const file = path.join(root, basename);
  let metadata;
  try {
    metadata = await lstat(file);
  } catch {
    throw new Error(
      `Linux ${artifactArch} evidence is missing ${path.relative(process.cwd(), file)}`,
    );
  }
  invariant(
    metadata.isFile(),
    `Linux ${artifactArch} evidence ${basename} is not a regular file`,
  );
  return file;
}

function parseChecksum(value, expectedFilename, label) {
  const lines = value.trim().split(/\r?\n/);
  invariant(lines.length === 1, `${label} must contain exactly one checksum`);
  const match = lines[0].match(/^([0-9a-f]{64})[ \t]+\*?(.+)$/);
  invariant(match, `${label} is not a valid SHA-256 checksum`);
  invariant(
    match[2] === expectedFilename,
    `${label} names ${match[2]}, expected ${expectedFilename}`,
  );
  return match[1];
}

function parseKeyValues(value) {
  const fields = {};
  for (const line of value.trim().split(/\r?\n/)) {
    const separator = line.indexOf("=");
    invariant(separator > 0, `Invalid evidence field: ${line}`);
    fields[line.slice(0, separator)] = line.slice(separator + 1);
  }
  return fields;
}

function verifyManifest(manifest, { version, candidateSha, dryRunId }) {
  invariant(manifest?.schemaVersion === 1, "Unsupported release manifest");
  invariant(
    manifest.sourceWorkflow === SOURCE_WORKFLOW,
    "Unexpected release source workflow",
  );
  invariant(manifest.workflowRunId === dryRunId, "Dry-run ID mismatch");
  invariant(manifest.candidateSha === candidateSha, "Candidate SHA mismatch");
  invariant(manifest.version === version, "Release manifest version mismatch");
  invariant(manifest.channel === "stable", "Release channel is not stable");
  invariant(
    manifest.publish === false,
    "Release manifest must come from a publish=false run",
  );
  invariant(
    Array.isArray(manifest.assets) && manifest.assets.length > 0,
    "Release manifest has no assets",
  );
}

async function verifyArchitecture({
  evidenceDir,
  manifest,
  version,
  candidateSha,
  dryRunId,
  audioQaRunId,
  architecture,
}) {
  const root = path.join(
    evidenceDir,
    architecture.artifactArch,
    "qa-artifacts",
    `linux-${architecture.artifactArch}`,
  );
  const required = (basename) =>
    requireFile(root, basename, architecture.artifactArch);

  const provenance = await readJson(await required("provenance.json"));
  invariant(
    provenance.application === APPLICATION,
    `Linux ${architecture.artifactArch} application mismatch`,
  );
  invariant(
    provenance.version === version,
    `Linux ${architecture.artifactArch} version mismatch`,
  );
  invariant(
    provenance.candidate_sha === candidateSha,
    `Linux ${architecture.artifactArch} candidate SHA mismatch`,
  );
  invariant(
    provenance.workflow_run_id === audioQaRunId,
    `Linux ${architecture.artifactArch} audio QA run mismatch`,
  );
  invariant(
    provenance.source_workflow_run_id === dryRunId,
    `Linux ${architecture.artifactArch} dry-run mismatch`,
  );
  invariant(
    provenance.public_platform === architecture.publicPlatform,
    `Linux ${architecture.artifactArch} platform mismatch`,
  );
  invariant(
    typeof provenance.crabnebula_asset_id === "string" &&
      /^[A-Za-z0-9_-]+$/.test(provenance.crabnebula_asset_id),
    `Linux ${architecture.artifactArch} asset ID is invalid`,
  );
  invariant(
    Number.isSafeInteger(provenance.crabnebula_asset_size) &&
      provenance.crabnebula_asset_size > 0,
    `Linux ${architecture.artifactArch} asset size is invalid`,
  );
  const expectedSha256 = normalizeSha256(
    provenance.expected_sha256,
    `Linux ${architecture.artifactArch} expected hash`,
  );

  const matchingAssets = manifest.assets.filter(
    (asset) => asset.id === provenance.crabnebula_asset_id,
  );
  invariant(
    matchingAssets.length === 1,
    `Linux ${architecture.artifactArch} asset is not unique in the release manifest`,
  );
  const manifestAsset = matchingAssets[0];
  invariant(
    manifestAsset.publicPlatform === architecture.publicPlatform &&
      manifestAsset.size === provenance.crabnebula_asset_size &&
      manifestAsset.sha256 === expectedSha256,
    `Linux ${architecture.artifactArch} asset does not match release provenance`,
  );

  const embeddedManifest = await readJson(
    await required(path.join("release-provenance", "manifest.json")),
  );
  invariant(
    JSON.stringify(embeddedManifest) === JSON.stringify(manifest),
    `Linux ${architecture.artifactArch} embedded release provenance mismatch`,
  );

  const crabNebulaAsset = await readJson(
    await required("crabnebula-asset.json"),
  );
  invariant(
    crabNebulaAsset.id === provenance.crabnebula_asset_id &&
      typeof crabNebulaAsset.filename === "string" &&
      crabNebulaAsset.filename.length > 0 &&
      crabNebulaAsset.publicPlatform === architecture.publicPlatform &&
      (crabNebulaAsset.updatePlatform ?? null) ===
        (manifestAsset.updatePlatform ?? null) &&
      Number(crabNebulaAsset.size) === provenance.crabnebula_asset_size,
    `Linux ${architecture.artifactArch} CrabNebula metadata mismatch`,
  );

  const downloadSha256 = parseChecksum(
    await readFile(await required("download.sha256"), "utf8"),
    `candidate-linux-${architecture.artifactArch}.deb`,
    `Linux ${architecture.artifactArch} download checksum`,
  );
  const testedSha256 = parseChecksum(
    await readFile(await required("candidate.deb.sha256"), "utf8"),
    "candidate.deb",
    `Linux ${architecture.artifactArch} tested-package checksum`,
  );
  invariant(
    downloadSha256 === expectedSha256 && testedSha256 === expectedSha256,
    `Linux ${architecture.artifactArch} tested package hash mismatch`,
  );

  const packageFields = parseKeyValues(
    await readFile(await required("debian-package.txt"), "utf8"),
  );
  invariant(
    packageFields.package === "anarlog" &&
      packageFields.version === version &&
      packageFields.architecture === architecture.debianArch,
    `Linux ${architecture.artifactArch} Debian package identity mismatch`,
  );

  const analysis = await readJson(await required("analysis.json"));
  const thresholdNames = Object.keys(ANALYSIS_THRESHOLDS).sort();
  const metricNames = Object.keys(analysis.metrics ?? {}).sort();
  invariant(
    analysis.status === "pass" &&
      analysis.scope === ANALYSIS_SCOPE &&
      Array.isArray(analysis.failures) &&
      analysis.failures.length === 0 &&
      JSON.stringify(Object.keys(analysis.thresholds ?? {}).sort()) ===
        JSON.stringify(thresholdNames) &&
      thresholdNames.every(
        (threshold) =>
          analysis.thresholds[threshold] === ANALYSIS_THRESHOLDS[threshold],
      ) &&
      JSON.stringify(metricNames) ===
        JSON.stringify([...ANALYSIS_METRICS].sort()) &&
      ANALYSIS_METRICS.every(
        (metric) =>
          typeof analysis.metrics?.[metric] === "number" &&
          Number.isFinite(analysis.metrics[metric]),
      ),
    `Linux ${architecture.artifactArch} audio analysis did not pass`,
  );

  const phases = await readJson(await required("phases.json"));
  const phaseList = Array.isArray(phases.phases) ? phases.phases : [];
  const phaseNames = phaseList.map((phase) => phase?.name).sort();
  const validIntervals = phaseList.every(
    (phase) =>
      typeof phase?.start_seconds === "number" &&
      Number.isFinite(phase.start_seconds) &&
      typeof phase?.end_seconds === "number" &&
      Number.isFinite(phase.end_seconds) &&
      phase.end_seconds - phase.start_seconds >= 5,
  );
  const latestPhaseEnd = Math.max(
    ...phaseList.map((phase) => phase?.end_seconds ?? Number.POSITIVE_INFINITY),
  );
  invariant(
    phases.schema_version === 1 &&
      phases.no_aec === true &&
      typeof phases.recording_stop_seconds === "number" &&
      Number.isFinite(phases.recording_stop_seconds) &&
      validIntervals &&
      phases.recording_stop_seconds > latestPhaseEnd &&
      JSON.stringify(phaseNames) ===
        JSON.stringify(["both", "mic_only", "system_only"]),
    `Linux ${architecture.artifactArch} phase evidence is invalid`,
  );

  const captureErrors = await readFile(
    await required("capture-errors.txt"),
    "utf8",
  );
  invariant(
    captureErrors.length === 0,
    `Linux ${architecture.artifactArch} evidence contains capture errors`,
  );
  const backendEvents = await readFile(
    await required("capture-backend-events.txt"),
    "utf8",
  );
  invariant(
    backendEvents.includes("pipewire_capture_initialized") &&
      !backendEvents.includes("pipewire_capture_unavailable") &&
      !backendEvents.includes("pulseaudio_capture_initialized"),
    `Linux ${architecture.artifactArch} did not prove direct PipeWire capture`,
  );

  for (const basename of ["audio_mic.wav", "audio_spk.wav"]) {
    const file = await required(basename);
    const header = await readFile(file);
    invariant(
      header.length > 44 &&
        header.subarray(0, 4).toString("ascii") === "RIFF" &&
        header.subarray(8, 12).toString("ascii") === "WAVE",
      `Linux ${architecture.artifactArch} ${basename} is not a nonempty WAV`,
    );
  }

  return {
    artifactArch: architecture.artifactArch,
    publicPlatform: architecture.publicPlatform,
    sha256: expectedSha256,
  };
}

export async function verifyLinuxAudioQaEvidence({
  evidenceDir,
  manifest,
  version,
  candidateSha,
  dryRunId,
  audioQaRunId,
}) {
  const normalized = {
    version: normalizeVersion(version),
    candidateSha: normalizeSha(candidateSha),
    dryRunId: normalizeRunId(dryRunId, "Dry-run ID"),
    audioQaRunId: normalizeRunId(audioQaRunId, "Audio QA run ID"),
  };
  verifyManifest(manifest, normalized);

  const results = [];
  for (const architecture of ARCHITECTURES) {
    results.push(
      await verifyArchitecture({
        evidenceDir,
        manifest,
        ...normalized,
        architecture,
      }),
    );
  }
  return results;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const results = await verifyLinuxAudioQaEvidence({
    evidenceDir: args["evidence-dir"],
    manifest: await readJson(args.manifest),
    version: args.version,
    candidateSha: args["candidate-sha"],
    dryRunId: args["dry-run-id"],
    audioQaRunId: args["audio-qa-run-id"],
  });
  console.log(
    `Verified Linux audio QA evidence for ${results.map((result) => result.artifactArch).join(" and ")}`,
  );
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
