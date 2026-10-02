import { describe, expect, it } from "vitest";

import { getLiveTranscriptPausedMessage } from "./live-transcript-interrupted";

describe("getLiveTranscriptPausedMessage", () => {
  it("names the provider when it reports an outage", () => {
    expect(
      getLiveTranscriptPausedMessage({
        degraded: { type: "stream_error", message: "deepgram: overloaded" },
        sttProvider: "deepgram",
        online: true,
      }),
    ).toMatch(/because Deepgram is having an outage\./);
  });

  it("separates an upstream outage from Anarlog server issues on cloud", () => {
    const upstream = getLiveTranscriptPausedMessage({
      degraded: {
        type: "stream_error",
        message: "RemoteClosed(code=Some(4500), reason=provider error)",
      },
      sttProvider: "anarlog",
      online: true,
    });
    const server = getLiveTranscriptPausedMessage({
      degraded: {
        type: "stream_error",
        message: "RemoteClosed(code=Some(1011), reason=)",
      },
      sttProvider: "anarlog",
      online: true,
    });

    expect(upstream).toMatch(/our speech-to-text provider is having an outage/);
    expect(server).toMatch(/Anarlog's transcription server is having issues/);
  });

  it("blames the connection when offline", () => {
    expect(
      getLiveTranscriptPausedMessage({
        degraded: { type: "connection_timeout" },
        sttProvider: "anarlog",
        online: false,
      }),
    ).toMatch(/because you're offline\./);
  });

  it("reports unreachable direct providers", () => {
    expect(
      getLiveTranscriptPausedMessage({
        degraded: { type: "connection_timeout" },
        sttProvider: "deepgram",
        online: true,
      }),
    ).toMatch(/Anarlog can't reach Deepgram/);
  });

  it("does not blame the network for on-device models", () => {
    expect(
      getLiveTranscriptPausedMessage({
        degraded: { type: "connection_timeout" },
        sttProvider: "soniqo",
        online: false,
      }),
    ).toMatch(/the local transcription model stopped responding/);
  });
});
