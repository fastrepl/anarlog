import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, it, vi } from "vitest";

import { DateEditor, dateEditorStyles } from "./date";
import { MetadataButton, metadataStyles } from "./index";

import { expectStyle } from "~/session/stylex-test";

const mocks = vi.hoisted(() => ({
  createdAt: "2026-07-02T03:53:00.000Z" as unknown,
  setCreatedAt: vi.fn(),
  sessionEvent: null as unknown,
}));

const lingui = vi.hoisted(() => {
  const t = (input: TemplateStringsArray | string, ...values: unknown[]) => {
    if (typeof input === "string") {
      return input;
    }

    return Array.from(input).reduce(
      (text, part, index) => `${text}${part}${values[index] ?? ""}`,
      "",
    );
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    t: lingui.t,
  }),
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: {
    openUrl: vi.fn(),
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => undefined,
}));

vi.mock("~/session/hooks/useSessionEvent", () => ({
  useSessionEvent: () => mocks.sessionEvent,
}));

vi.mock("~/session/queries", () => ({
  useSession: () => ({ created_at: mocks.createdAt }),
  useUpdateSession: () => mocks.setCreatedAt,
}));

vi.mock("./participants", () => ({
  ParticipantsDisplay: () => null,
}));

describe("Metadata controls", () => {
  beforeEach(() => {
    mocks.createdAt = "2026-07-02T03:53:00.000Z";
    mocks.setCreatedAt.mockClear();
    mocks.sessionEvent = null;
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the metadata calendar trigger as a circle", () => {
    render(<MetadataButton sessionId="session-1" />);

    const metadataButton = screen.getByRole("button", {
      name: "Open note metadata",
    });

    expectStyle(metadataButton, metadataStyles.trigger);
  });

  it("renders date edit action buttons as circles", () => {
    render(<DateEditor sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Edit date" }));

    expectStyle(
      screen.getByRole("button", { name: "Cancel date edit" }),
      dateEditorStyles.iconButton,
    );
    expectStyle(
      screen.getByRole("button", { name: "Save date" }),
      dateEditorStyles.iconButton,
    );
  });
});
