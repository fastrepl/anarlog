import { beforeEach, describe, expect, it, vi } from "vitest";

const { audioPeaks } = vi.hoisted(() => ({ audioPeaks: vi.fn() }));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: { audioPeaks },
}));

import {
  isUsablePeaks,
  loadSessionPeaks,
  loadWaveform,
  prepareSessionPeaks,
  type WaveformPeaks,
} from "./waveform";

const peaks: WaveformPeaks = {
  duration: 12.5,
  channels: [
    [0.1, 0.4],
    [0.2, 0.3],
  ],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

function setup(status = 200) {
  const blob = new Blob(["audio"], { type: "audio/mpeg" });
  const media = { src: "", canPlayType: vi.fn().mockReturnValue("maybe") };
  const ws = {
    load: vi.fn().mockResolvedValue(undefined),
    getMediaElement: vi.fn().mockReturnValue(media),
  };
  const fetchAudio = vi.fn().mockResolvedValue({
    status,
    blob: () => Promise.resolve(blob),
  });
  return { blob, media, ws, fetchAudio };
}

beforeEach(() => {
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:recording");
});

describe("loadWaveform", () => {
  it("draws native peaks on the fetched recording without decoding it", async () => {
    const { media, ws, fetchAudio } = setup();
    const controller = new AbortController();
    const loadPeaks = vi.fn().mockResolvedValue(peaks);

    await loadWaveform(ws, {
      url: "asset://audio.mp3",
      sessionId: "session",
      signal: controller.signal,
      fetchAudio,
      loadPeaks,
    });

    expect(loadPeaks).toHaveBeenCalledWith("session");
    expect(fetchAudio).toHaveBeenCalledWith("asset://audio.mp3", {
      signal: controller.signal,
    });
    expect(media.src).toBe("blob:recording");
    expect(ws.load).toHaveBeenCalledWith(
      "blob:recording",
      peaks.channels,
      12.5,
    );
  });

  it("makes the recording playable before peaks are ready", async () => {
    const { media, ws, fetchAudio } = setup();
    const pending = deferred<WaveformPeaks | null>();

    const loading = loadWaveform(ws, {
      url: "asset://audio.mp3",
      sessionId: "session",
      signal: new AbortController().signal,
      fetchAudio,
      loadPeaks: () => pending.promise,
    });

    await vi.waitFor(() => expect(media.src).toBe("blob:recording"));
    expect(ws.load).not.toHaveBeenCalled();

    pending.resolve(peaks);
    await loading;
    expect(ws.load).toHaveBeenCalledWith(
      "blob:recording",
      peaks.channels,
      12.5,
    );
  });

  it("streams from the url when the blob type is not playable", async () => {
    const { media, ws, fetchAudio } = setup();
    media.canPlayType.mockReturnValue("");

    await loadWaveform(ws, {
      url: "asset://audio.mp3",
      sessionId: "session",
      signal: new AbortController().signal,
      fetchAudio,
      loadPeaks: vi.fn().mockResolvedValue(peaks),
    });

    expect(media.src).toBe("asset://audio.mp3");
    expect(ws.load).toHaveBeenCalledWith(
      "asset://audio.mp3",
      peaks.channels,
      12.5,
    );
  });

  it("falls back to wavesurfer decoding when peaks are unavailable", async () => {
    const { ws, fetchAudio } = setup();

    await loadWaveform(ws, {
      url: "asset://audio.mp3",
      sessionId: "session",
      signal: new AbortController().signal,
      fetchAudio,
      loadPeaks: vi.fn().mockResolvedValue(null),
    });

    expect(ws.load).toHaveBeenCalledWith("blob:recording");
  });

  it("does not load after the player was torn down", async () => {
    const { media, ws, fetchAudio } = setup();
    const controller = new AbortController();
    const loading = loadWaveform(ws, {
      url: "asset://audio.mp3",
      sessionId: "session",
      signal: controller.signal,
      fetchAudio,
      loadPeaks: vi.fn().mockResolvedValue(peaks),
    });
    controller.abort();
    await loading;

    expect(media.src).toBe("");
    expect(ws.load).not.toHaveBeenCalled();
  });

  it("rejects when the recording cannot be fetched", async () => {
    const { ws, fetchAudio } = setup(404);

    await expect(
      loadWaveform(ws, {
        url: "asset://audio.mp3",
        sessionId: "session",
        signal: new AbortController().signal,
        fetchAudio,
        loadPeaks: vi.fn().mockResolvedValue(peaks),
      }),
    ).rejects.toThrow("404");
    expect(ws.load).not.toHaveBeenCalled();
  });
});

describe("loadSessionPeaks", () => {
  it("shares one native request per recording while it is in flight", async () => {
    const pending = deferred<{ status: "ok"; data: WaveformPeaks }>();
    audioPeaks.mockReset().mockReturnValue(pending.promise);

    const first = loadSessionPeaks("session");
    const second = loadSessionPeaks("session");
    pending.resolve({ status: "ok", data: peaks });

    await expect(first).resolves.toEqual(peaks);
    await expect(second).resolves.toEqual(peaks);
    expect(audioPeaks).toHaveBeenCalledTimes(1);

    audioPeaks.mockResolvedValue({ status: "error", error: "failed" });
    await expect(loadSessionPeaks("session")).resolves.toBeNull();
    expect(audioPeaks).toHaveBeenCalledTimes(2);
  });

  it("treats command failures as missing peaks", async () => {
    audioPeaks.mockReset().mockRejectedValue(new Error("ipc"));
    await expect(loadSessionPeaks("other")).resolves.toBeNull();
  });
});

describe("isUsablePeaks", () => {
  it("rejects empty or durationless peaks", () => {
    expect(isUsablePeaks(peaks)).toBe(true);
    expect(isUsablePeaks(null)).toBe(false);
    expect(isUsablePeaks({ duration: 0, channels: [[0.1]] })).toBe(false);
    expect(isUsablePeaks({ duration: 1, channels: [] })).toBe(false);
    expect(isUsablePeaks({ duration: 1, channels: [[]] })).toBe(false);
  });
});

describe("prepareSessionPeaks", () => {
  it("does not share its request with later waveform loads", async () => {
    audioPeaks.mockReset();
    const warm = deferred<unknown>();
    audioPeaks
      .mockReturnValueOnce(warm.promise)
      .mockResolvedValueOnce({ status: "ok", data: peaks });

    const preparing = prepareSessionPeaks("recorded");
    await expect(loadSessionPeaks("recorded")).resolves.toEqual(peaks);
    expect(audioPeaks).toHaveBeenCalledTimes(2);

    warm.resolve({ status: "ok", data: peaks });
    await preparing;
  });

  it("swallows native failures", async () => {
    audioPeaks.mockReset();
    audioPeaks.mockRejectedValueOnce(new Error("ipc down"));

    await expect(prepareSessionPeaks("failing")).resolves.toBeUndefined();
  });
});
