import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterAll, afterEach, describe, expect, it, vi } from "vitest";

import { EditorErrorBoundary } from "./editor-error-boundary";

const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
const renderOptions = {
  onCaughtError: () => {},
  onRecoverableError: () => {},
} satisfies Parameters<typeof render>[1];

afterEach(() => {
  cleanup();
  consoleError.mockClear();
});

afterAll(() => {
  consoleError.mockRestore();
});

describe("EditorErrorBoundary", () => {
  it("automatically remounts the editor once after a render failure", async () => {
    let renderCount = 0;

    function FlakyEditor() {
      renderCount += 1;
      if (renderCount === 1) {
        throw new Error("first render failed");
      }

      return <div>editor ready</div>;
    }

    render(
      <EditorErrorBoundary>
        <FlakyEditor />
      </EditorErrorBoundary>,
      renderOptions,
    );

    await waitFor(() => {
      expect(screen.getByText("editor ready")).toBeTruthy();
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps a manual reload path after repeated render failures", async () => {
    function BrokenEditor() {
      throw new Error("render failed");
    }

    render(
      <EditorErrorBoundary>
        <BrokenEditor />
      </EditorErrorBoundary>,
      renderOptions,
    );

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The editor failed to render");

    fireEvent.click(screen.getByRole("button", { name: "Reload editor" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeTruthy();
    });
  });

  it("resets after the editor identity changes", async () => {
    function BrokenEditor() {
      throw new Error("render failed");
    }

    const view = render(
      <EditorErrorBoundary resetKey="session-a">
        <BrokenEditor />
      </EditorErrorBoundary>,
      renderOptions,
    );

    await screen.findByRole("alert");

    view.rerender(
      <EditorErrorBoundary resetKey="session-b">
        <div>new editor</div>
      </EditorErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.getByText("new editor")).toBeTruthy();
    });
  });
});
