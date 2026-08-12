import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AvatarUploadButton } from "./contact-avatar";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("AvatarUploadButton", () => {
  it("crops uploaded avatars to a 70 by 70 JPEG", async () => {
    const context = {
      drawImage: vi.fn(),
      fillRect: vi.fn(),
      fillStyle: "",
      imageSmoothingQuality: "low",
    };
    const onUpload = vi.fn();

    vi.stubGlobal("URL", {
      createObjectURL: vi.fn(() => "blob:avatar"),
      revokeObjectURL: vi.fn(),
    });
    vi.stubGlobal(
      "Image",
      class {
        naturalHeight = 200;
        naturalWidth = 400;
        onerror: (() => void) | null = null;
        onload: (() => void) | null = null;

        set src(_value: string) {
          queueMicrotask(() => this.onload?.());
        }
      },
    );
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      context as unknown as CanvasRenderingContext2D,
    );
    vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockImplementation(
      function (this: HTMLCanvasElement, type, quality) {
        expect(this.width).toBe(70);
        expect(this.height).toBe(70);
        expect(type).toBe("image/jpeg");
        expect(quality).toBe(0.85);
        return "data:image/jpeg;base64,compressed";
      },
    );

    const { container } = render(
      <AvatarUploadButton label="Change photo" onUpload={onUpload}>
        <span>Avatar</span>
      </AvatarUploadButton>,
    );
    const input =
      container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    if (!input) return;

    fireEvent.change(input, {
      target: {
        files: [new File(["avatar"], "avatar.png", { type: "image/png" })],
      },
    });

    await waitFor(() => {
      expect(onUpload).toHaveBeenCalledWith(
        "data:image/jpeg;base64,compressed",
      );
    });
    expect(context.drawImage).toHaveBeenCalledWith(
      expect.anything(),
      100,
      0,
      200,
      200,
      0,
      0,
      70,
      70,
    );
  });
});
