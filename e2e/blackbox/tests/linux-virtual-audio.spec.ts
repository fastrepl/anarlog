import { $, browser } from "@wdio/globals";
import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { performance } from "node:perf_hooks";

const virtualAudioEnabled = process.env.E2E_VIRTUAL_AUDIO === "1";
const describeVirtualAudio = virtualAudioEnabled ? describe : describe.skip;

describeVirtualAudio("Linux virtual audio capture", () => {
  it("persists separated microphone and system-audio tracks", async () => {
    if (process.platform !== "linux") {
      throw new Error("E2E_VIRTUAL_AUDIO is only supported on Linux");
    }
    if (process.env.NO_AEC !== "1") {
      throw new Error("Linux virtual-audio QA must run with NO_AEC=1");
    }

    const micSignal = requiredEnvironmentPath("QA_MIC_SIGNAL");
    const systemSignal = requiredEnvironmentPath("QA_SYSTEM_SIGNAL");
    const phasesPath = requiredEnvironmentPath("QA_PHASES_PATH");
    const micSink = process.env.QA_MIC_SINK ?? "qa_mic_bus";
    const systemSink = process.env.QA_SYSTEM_SINK ?? "qa_system";

    await mkdir(path.dirname(phasesPath), { recursive: true });
    await closeChangelogIfVisible();

    let startRecording = await $(
      '//button[.//span[normalize-space()="Start Recording"]]',
    );
    await startRecording.waitForDisplayed({ timeout: 30_000 });
    await startRecording.waitForEnabled({ timeout: 30_000 });
    await closeChangelogIfVisible();
    startRecording = await $(
      '//button[.//span[normalize-space()="Start Recording"]]',
    );
    await startRecording.waitForDisplayed({ timeout: 30_000 });
    await startRecording.waitForEnabled({ timeout: 30_000 });
    await startRecording.click();

    await browser.waitUntil(
      async () => {
        const stopListening = await $('button[title="Stop listening"]');
        const record = await $('button[title="Record"]');
        return (
          (await stopListening.isDisplayed()) || (await record.isDisplayed())
        );
      },
      {
        timeout: 30_000,
        timeoutMsg: "session recording controls did not appear",
      },
    );

    const record = await $('button[title="Record"]');
    if (await record.isDisplayed()) {
      await record.waitForEnabled({ timeout: 30_000 });
      await record.click();
    }

    const stopListening = await $('button[title="Stop listening"]');
    await stopListening.waitForDisplayed({ timeout: 30_000 });

    const anchor = performance.now();
    const phases: {
      name: "mic_only" | "system_only" | "both";
      start_seconds: number;
      end_seconds: number;
    }[] = [];

    await pause(2_500);
    phases.push(
      await playPhase("mic_only", anchor, [
        { sink: micSink, signal: micSignal },
      ]),
    );
    await persistPhases(phasesPath, phases);

    await pause(2_500);
    phases.push(
      await playPhase("system_only", anchor, [
        { sink: systemSink, signal: systemSignal },
      ]),
    );
    await persistPhases(phasesPath, phases);

    await pause(2_500);
    phases.push(
      await playPhase("both", anchor, [
        { sink: micSink, signal: micSignal },
        { sink: systemSink, signal: systemSignal },
      ]),
    );
    await persistPhases(phasesPath, phases);

    await pause(2_500);
    await stopListening.click();
    const recordingStopSeconds = (performance.now() - anchor) / 1000;
    await stopListening.waitForDisplayed({
      timeout: 30_000,
      reverse: true,
    });
    const settledControl = await $(
      'button[title="Resume listening"]:not([disabled]), button[title="Record"]:not([disabled]), button[title="Stop transcription"]:not([disabled])',
    );
    await settledControl.waitForDisplayed({ timeout: 60_000 });
    await persistPhases(phasesPath, phases, recordingStopSeconds);
    await pause(1_000);
  });
});

async function closeChangelogIfVisible() {
  const closeChangelog = await $('button[aria-label="Close changelog"]');
  if (
    (await closeChangelog.isExisting()) &&
    (await closeChangelog.isDisplayed())
  ) {
    await closeChangelog.click();
  }
}

function requiredEnvironmentPath(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required for Linux virtual-audio QA`);
  }
  return path.resolve(value);
}

async function playPhase(
  name: "mic_only" | "system_only" | "both",
  anchor: number,
  streams: { sink: string; signal: string }[],
) {
  const startSeconds = (performance.now() - anchor) / 1000;
  await Promise.all(
    streams.map(({ sink, signal }) =>
      runProcess("paplay", [`--device=${sink}`, signal]),
    ),
  );
  return {
    name,
    start_seconds: startSeconds,
    end_seconds: (performance.now() - anchor) / 1000,
  };
}

async function persistPhases(
  phasesPath: string,
  phases: {
    name: "mic_only" | "system_only" | "both";
    start_seconds: number;
    end_seconds: number;
  }[],
  recordingStopSeconds?: number,
) {
  await writeFile(
    phasesPath,
    JSON.stringify(
      {
        schema_version: 1,
        recording_anchor: "stop-listening control became visible",
        no_aec: true,
        scope:
          "Virtual routing, capture, separation, and persistence only; this is not an AEC test.",
        phases,
        ...(recordingStopSeconds === undefined
          ? {}
          : { recording_stop_seconds: recordingStopSeconds }),
      },
      null,
      2,
    ),
  );
}

async function runProcess(command: string, args: string[]) {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit" });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(
        new Error(
          `${command} exited with ${code ?? `signal ${signal ?? "unknown"}`}`,
        ),
      );
    });
  });
}

async function pause(durationMs: number) {
  await new Promise((resolve) => setTimeout(resolve, durationMs));
}
