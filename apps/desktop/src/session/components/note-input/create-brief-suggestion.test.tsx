import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings[0] }),
}));

import {
  CreateBriefSuggestion,
  createBriefSuggestionStyles,
} from "./create-brief-suggestion";

import { expectStyle } from "~/session/stylex-test";

describe("CreateBriefSuggestion", () => {
  afterEach(cleanup);

  it("offers a brief in the empty memo and creates it on click", () => {
    const onCreate = vi.fn();

    render(<CreateBriefSuggestion onCreate={onCreate} />);

    expect(screen.queryByText("Prepare for this meeting")).toBeNull();
    const button = screen.getByRole("button", {
      name: "Create a brief to prepare this meeting",
    });
    expectStyle(button, createBriefSuggestionStyles.button);
    fireEvent.click(button);

    expect(onCreate).toHaveBeenCalledOnce();
  });
});
