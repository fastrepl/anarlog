import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AvatarUploadButton } from "./contact-avatar";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("AvatarUploadButton", () => {
  it.each([
    { width: 800, height: 400, output: 256 },
    { width: 128, height: 64, output: 64 },
    { width: 64, height: 128, output: 64 },
    { width: 128, height: 128, output: 128 },
  ])(
    "center-crops $width × $height avatars to $output pixels without upscaling",
    async ({ width, height, output }) => {
      const side = Math.min(width, height);
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
          naturalHeight = height;
          naturalWidth = width;
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
          expect(this.width).toBe(output);
          expect(this.height).toBe(output);
          expect(type).toBe("image/jpeg");
          expect(quality).toBe(0.95);
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
        (width - side) / 2,
        (height - side) / 2,
        side,
        side,
        0,
        0,
        output,
        output,
      );
    },
  );
});
