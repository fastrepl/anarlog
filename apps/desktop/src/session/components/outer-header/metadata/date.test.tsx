import { render, screen } from "@testing-library/react";
import type React from "react";
import { describe, expect, it, vi } from "vitest";

const { useSessionCellOptionalMock, safeFormatMock } = vi.hoisted(() => ({
  useSessionCellOptionalMock: vi.fn(() => undefined),
  safeFormatMock: vi.fn(() => "Rendered date"),
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock("@hypr/ui/components/ui/input", () => ({
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => (
    <input {...props} />
  ),
}));

vi.mock("@hypr/utils", () => ({
  format: vi.fn(),
  safeParseDate: vi.fn(() => null),
  safeFormat: safeFormatMock,
}));

vi.mock("~/session/hooks/storage", () => ({
  useSessionCellOptional: useSessionCellOptionalMock,
  useSetSessionCreatedAt: vi.fn(() => vi.fn()),
}));

import { DateEditor } from "./date";

describe("DateEditor", () => {
  it("uses Date fallback when created_at is missing", () => {
    render(<DateEditor sessionId="session-1" />);

    expect(safeFormatMock).toHaveBeenCalledWith(
      expect.any(Date),
      "MMM d, yyyy h:mm a",
      "Unknown date",
    );
    expect(screen.getByText("Rendered date")).not.toBeNull();
  });
});
