import { describe, expect, it } from "vitest";

import {
  type GeneralState,
  initialGeneralState,
  noteLiveTranscriptActivity,
  TRANSCRIPTION_STALL_AUDIBLE_SECONDS,
  tickTranscriptionStallWatchdog,
} from "./general-shared";

function createActiveLive(): GeneralState["live"] {
  return {
    ...initialGeneralState.live,
    status: "active",
    sessionId: "session-1",
    requestedLiveTranscription: true,
    liveTranscriptionActive: true,
    amplitude: { mic: 0.4, speaker: 0.4 },
    finalizingBySession: {},
    eventUnlistenersBySession: {},
  };
}

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

    noteLiveTranscriptActivity(live);
    expect(live.stallAudibleSeconds).toBe(0);

    expect(tickTranscriptionStallWatchdog(live)).toBe(false);
    expect(live.transcriptionStalled).toBe(false);
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

    noteLiveTranscriptActivity(live);
    expect(live.transcriptionStalled).toBe(false);
    expect(live.needsBatchRepair).toBe(true);
  });
});
