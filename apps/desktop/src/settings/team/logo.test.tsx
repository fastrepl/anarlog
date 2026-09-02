import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { isWorkspaceLogoDataUrl } from "./logo";
import { WorkspaceLogoButton } from "./logo-button";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("isWorkspaceLogoDataUrl", () => {
  it("accepts a bounded JPEG data URL", () => {
    expect(isWorkspaceLogoDataUrl("data:image/jpeg;base64,/9j/4AAQ")).toBe(
      true,
    );
  });

  it("rejects non-JPEG and oversized payloads", () => {
    expect(isWorkspaceLogoDataUrl("data:image/png;base64,AAAA")).toBe(false);
    expect(isWorkspaceLogoDataUrl("javascript:alert(1)")).toBe(false);
    expect(
      isWorkspaceLogoDataUrl(`data:image/jpeg;base64,${"A".repeat(120_000)}`),
    ).toBe(false);
  });
});

describe("WorkspaceLogoButton", () => {
  it("crops uploaded logos to a 128 by 128 JPEG", async () => {
    const context = {
      drawImage: vi.fn(),
      fillRect: vi.fn(),
      fillStyle: "",
      imageSmoothingQuality: "low",
    };
    const onUpload = vi.fn();
    const jpeg = "data:image/jpeg;base64,/9j/4AAQ";

    vi.stubGlobal("URL", {
      createObjectURL: vi.fn(() => "blob:logo"),
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
        expect(this.width).toBe(128);
        expect(this.height).toBe(128);
        expect(type).toBe("image/jpeg");
        expect(quality).toBe(0.85);
        return jpeg;
      },
    );

    const { container } = render(
      <WorkspaceLogoButton
        logoDataUrl={null}
        label="Change workspace logo"
        removeLabel="Remove workspace logo"
        canManage
        pending={false}
        onUpload={onUpload}
        onRemove={vi.fn()}
      />,
    );
    const input =
      container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    if (!input) return;

    fireEvent.change(input, {
      target: {
        files: [new File(["logo"], "logo.png", { type: "image/png" })],
      },
    });

    await waitFor(() => {
      expect(onUpload).toHaveBeenCalledWith(jpeg);
    });
    expect(context.drawImage).toHaveBeenCalledWith(
      expect.anything(),
      100,
      0,
      200,
      200,
      0,
      0,
      128,
      128,
    );
  });

  it("does not offer an upload control to members", () => {
    const { container } = render(
      <WorkspaceLogoButton
        logoDataUrl={null}
        label="Change workspace logo"
        removeLabel="Remove workspace logo"
        canManage={false}
        pending={false}
        onUpload={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    expect(container.querySelector('input[type="file"]')).toBeNull();
    expect(
      container.querySelector('button[aria-label="Change workspace logo"]'),
    ).toBeNull();
  });

  it("clears an existing logo from the remove control", () => {
    const onRemove = vi.fn();
    const { getByRole } = render(
      <WorkspaceLogoButton
        logoDataUrl="data:image/jpeg;base64,/9j/4AAQ"
        label="Change workspace logo"
        removeLabel="Remove workspace logo"
        canManage
        pending={false}
        onUpload={vi.fn()}
        onRemove={onRemove}
      />,
    );

    fireEvent.click(getByRole("button", { name: "Remove workspace logo" }));
    expect(onRemove).toHaveBeenCalledOnce();
  });
});
