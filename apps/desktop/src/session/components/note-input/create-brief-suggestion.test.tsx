import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings[0] }),
}));

import { CreateBriefSuggestion } from "./create-brief-suggestion";

describe("CreateBriefSuggestion", () => {
  afterEach(cleanup);

  it("offers a brief in the empty memo and creates it on click", () => {
    const onCreate = vi.fn();

    render(<CreateBriefSuggestion onCreate={onCreate} />);

    const heading = screen.getByText("Prepare for this meeting");
    expect(heading.parentElement?.className).toContain("mb-6");
    const button = screen.getByRole("button", {
      name: "Want me to create a brief to help you prepare?",
    });
    expect(button.className).toContain("-ml-2");
    expect(button.className).toContain("h-8");
    expect(button.className).toContain("text-muted-foreground");
    fireEvent.click(button);

    expect(onCreate).toHaveBeenCalledOnce();
  });
});
