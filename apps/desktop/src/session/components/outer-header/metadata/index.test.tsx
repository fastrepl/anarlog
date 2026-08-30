import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DateEditor } from "./date";
import { MetadataButton } from "./index";

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

  it("uses a narrow floating panel", () => {
    render(<MetadataButton sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Open note metadata" }));

    const content = screen
      .getByRole("button", { name: "Edit date" })
      .closest("[data-radix-popper-content-wrapper] > *");

    expect(content?.className.split(/\s+/) ?? []).toContain("w-72");
  });

  it("renders the metadata calendar trigger as a circle", () => {
    render(<MetadataButton sessionId="session-1" />);

    const metadataButton = screen.getByRole("button", {
      name: "Open note metadata",
    });

    expect(metadataButton.className).toContain("size-7");
    expect(metadataButton.className).toContain("rounded-full");
    expect(metadataButton.className).toContain("[&_svg]:size-4");
    expect(
      metadataButton.querySelector("svg")?.getAttribute("class"),
    ).toContain("size-4");
  });

  it("renders date edit action buttons as circles", () => {
    render(<DateEditor sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Edit date" }));

    expect(
      screen.getByRole("button", { name: "Cancel date edit" }).className,
    ).toContain("rounded-full");
    expect(
      screen.getByRole("button", { name: "Save date" }).className,
    ).toContain("rounded-full");
  });

  it("keeps date edit actions available in the narrow panel", () => {
    render(<MetadataButton sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Open note metadata" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit date" }));

    const input = document.querySelector('input[type="datetime-local"]');
    const actions = screen.getByRole("button", {
      name: "Save date",
    }).parentElement;

    expect(input).not.toBeNull();
    expect(input?.className).toContain("w-full");
    expect(input?.className).toContain("min-w-0");
    expect(actions?.className).toContain("shrink-0");
    expect(actions?.className).toContain("justify-end");
    expect(actions?.parentElement?.className).toContain("flex-col");
    expect(
      screen.getByRole("button", { name: "Cancel date edit" }),
    ).not.toBeNull();
  });
});
