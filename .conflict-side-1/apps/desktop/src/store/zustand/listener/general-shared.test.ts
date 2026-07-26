import { describe, expect, it } from "vitest";

import {
  type GeneralState,
  initialGeneralState,
  markLiveCaptureStarted,
  markLiveFinalizing,
  markLiveStartRequested,
  noteLiveTranscriptActivity,
  releaseLiveCaptureGeneration,
  TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS,
  TRANSCRIPTION_STALL_AUDIBLE_SECONDS,
  tickTranscriptionStallWatchdog,
} from "./general-shared";

function createLive(): GeneralState["live"] {
  return {
    ...initialGeneralState.live,
    captureGenerationBySession: {},
    finalizingBySession: {},
    eventUnlistenersBySession: {},
  };
}

function createActiveLive(): GeneralState["live"] {
  return {
    ...createLive(),
    status: "active",
    sessionId: "session-1",
    requestedLiveTranscription: true,
    liveTranscriptionActive: true,
    amplitude: { mic: 0.4, speaker: 0.4 },
  };
}

describe("markLiveCaptureStarted", () => {
  it("keeps one generation per capture across lifecycle observations", () => {
    const live = createLive();

    markLiveStartRequested(live, "session-1");
    expect(live.captureGenerationBySession["session-1"]).toBe(1);
    markLiveCaptureStarted(live, "session-1");
    expect(live.captureGenerationBySession["session-1"]).toBe(1);
    markLiveCaptureStarted(live, "session-1");
    markLiveFinalizing(live, "session-1");
    expect(live.captureGenerationBySession["session-1"]).toBe(1);

    releaseLiveCaptureGeneration(live, "session-1");
    markLiveStartRequested(live, "session-1");
    expect(live.captureGenerationBySession["session-1"]).toBe(2);
  });

  it("keeps a finalizing session stable while another capture starts", () => {
    const live = createLive();

    markLiveStartRequested(live, "session-a");
    markLiveCaptureStarted(live, "session-a");
    markLiveFinalizing(live, "session-a");
    markLiveStartRequested(live, "session-b");

    expect(live.captureGenerationBySession).toEqual({
      "session-a": 1,
      "session-b": 2,
    });
  });
});

describe("tickTranscriptionStallWatchdog", () => {
  it("flags a stalled live transcription after sustained audible silence", () => {
    const live = createActiveLive();

    let stalledAt: number | null = null;
    for (
      let second = 1;
      second <= TRANSCRIPTION_STALL_AUDIBLE_SECONDS + 5;
      second += 1
    ) {
      if (tickTranscriptionStallWatchdog(live)) {
        stalledAt = second;
        break;
      }
    }

    expect(stalledAt).toBe(TRANSCRIPTION_STALL_AUDIBLE_SECONDS);
    expect(live.transcriptionStalled).toBe(true);
    expect(live.needsBatchRepair).toBe(true);
  });

  it("only counts seconds with audible audio", () => {
    const live = createActiveLive();
    live.amplitude = { mic: 0, speaker: 0 };

    for (
      let second = 0;
      second < TRANSCRIPTION_STALL_AUDIBLE_SECONDS * 2;
      second += 1
    ) {
      expect(tickTranscriptionStallWatchdog(live)).toBe(false);
    }

    expect(live.transcriptionStalled).toBe(false);
    expect(live.needsBatchRepair).toBe(false);
    expect(live.stallAudibleSeconds).toBe(0);
  });

  it("resets the stall counter when transcript activity arrives", () => {
    const live = createActiveLive();

    for (
      let second = 0;
      second < TRANSCRIPTION_STALL_AUDIBLE_SECONDS - 1;
      second += 1
    ) {
      tickTranscriptionStallWatchdog(live);
    }
    expect(live.stallAudibleSeconds).toBe(
      TRANSCRIPTION_STALL_AUDIBLE_SECONDS - 1,
    );

    noteLiveTranscriptActivity(live, { hasFinalWords: true });
    expect(live.stallAudibleSeconds).toBe(0);
    expect(live.finalStallAudibleSeconds).toBe(0);

    expect(tickTranscriptionStallWatchdog(live)).toBe(false);
    expect(live.transcriptionStalled).toBe(false);
  });

  it("flags a stall when partials keep flowing but nothing finalizes", () => {
    const live = createActiveLive();

    let stalledAt: number | null = null;
    for (
      let second = 1;
      second <= TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS + 5;
      second += 1
    ) {
      if (tickTranscriptionStallWatchdog(live)) {
        stalledAt = second;
        break;
      }
      noteLiveTranscriptActivity(live, { hasFinalWords: false });
    }

    expect(stalledAt).toBe(TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS);
    expect(live.transcriptionStalled).toBe(true);
    expect(live.needsBatchRepair).toBe(true);
  });

  it("keeps the stalled flag until finalized words arrive", () => {
    const live = createActiveLive();
    live.transcriptionStalled = true;
    live.needsBatchRepair = true;

    noteLiveTranscriptActivity(live, { hasFinalWords: false });
    expect(live.transcriptionStalled).toBe(true);

    noteLiveTranscriptActivity(live, { hasFinalWords: true });
    expect(live.transcriptionStalled).toBe(false);
    expect(live.needsBatchRepair).toBe(true);
  });

  it("stays quiet for record-only sessions and repeated stalls", () => {
    const recordOnly = createActiveLive();
    recordOnly.requestedLiveTranscription = false;
    recordOnly.liveTranscriptionActive = false;
    expect(tickTranscriptionStallWatchdog(recordOnly)).toBe(false);

    const stalled = createActiveLive();
    stalled.transcriptionStalled = true;
    stalled.needsBatchRepair = true;
    expect(tickTranscriptionStallWatchdog(stalled)).toBe(false);
  });

  it("keeps watching audible speaker audio while the mic is muted", () => {
    const muted = createActiveLive();
    muted.muted = true;
    muted.amplitude = { mic: 0, speaker: 1 };

    tickTranscriptionStallWatchdog(muted);
    expect(muted.stallAudibleSeconds).toBe(1);
  });

  it("keeps the batch repair flag after transcript activity resumes", () => {
    const live = createActiveLive();

    for (
      let second = 0;
      second < TRANSCRIPTION_STALL_AUDIBLE_SECONDS;
      second += 1
    ) {
      tickTranscriptionStallWatchdog(live);
    }
    expect(live.needsBatchRepair).toBe(true);

    noteLiveTranscriptActivity(live, { hasFinalWords: true });
    expect(live.transcriptionStalled).toBe(false);
    expect(live.needsBatchRepair).toBe(true);
  });
});
