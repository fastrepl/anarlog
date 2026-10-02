import { describe, expect, it, vi } from "vitest";

import { configureCenteredPlayback } from "./playback";

function createFakeContext() {
  const destination = {};
  const gainNode = {
    channelCount: 2,
    channelCountMode: "max",
    channelInterpretation: "discrete",
    connect: vi.fn(),
  };
  const source = { connect: vi.fn() };
  const context = {
    close: vi.fn().mockResolvedValue(undefined),
    createGain: vi.fn(() => gainNode),
    createMediaElementSource: vi.fn(() => source),
    destination,
  };
  return { context, destination, gainNode, source };
}

describe("configureCenteredPlayback", () => {
  it("routes the media element through a centered mono gain node", () => {
    const { context, destination, gainNode, source } = createFakeContext();
    const media = {} as HTMLMediaElement;

    expect(
      configureCenteredPlayback(
        media,
        () => context as unknown as AudioContext,
      ),
    ).toBe(context);
    expect(context.createMediaElementSource).toHaveBeenCalledWith(media);
    expect(source.connect).toHaveBeenCalledWith(gainNode);
    expect(gainNode.connect).toHaveBeenCalledWith(destination);
    expect(gainNode).toMatchObject({
      channelCount: 1,
      channelCountMode: "explicit",
      channelInterpretation: "speakers",
    });
  });

  it("keeps plain media-element playback when Web Audio is unavailable", () => {
    expect(
      configureCenteredPlayback({} as HTMLMediaElement, () => {
        throw new Error("unsupported");
      }),
    ).toBeNull();
  });

  it("closes the context when the element cannot be routed", () => {
    const { context } = createFakeContext();
    context.createMediaElementSource.mockImplementation(() => {
      throw new Error("already connected");
    });

    expect(
      configureCenteredPlayback(
        {} as HTMLMediaElement,
        () => context as unknown as AudioContext,
      ),
    ).toBeNull();
    expect(context.close).toHaveBeenCalledTimes(1);
  });

  it("does not reroute the element when gain setup fails", () => {
    const { context } = createFakeContext();
    context.createGain.mockImplementation(() => {
      throw new Error("unsupported");
    });

    expect(
      configureCenteredPlayback(
        {} as HTMLMediaElement,
        () => context as unknown as AudioContext,
      ),
    ).toBeNull();
    expect(context.createMediaElementSource).not.toHaveBeenCalled();
  });
});
