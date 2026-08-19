import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BrandLoadingView } from "./brand-loading-view";

describe("BrandLoadingView", () => {
  afterEach(cleanup);

  it("shows the anarlog mark while loading", () => {
    render(<BrandLoadingView />);

    const status = screen.getByRole("status", { name: "Loading" });
    expect(status.querySelectorAll("svg")).toHaveLength(2);
    expect(
      screen.getByText("Updating your data. This may take a few minutes."),
    ).toBeTruthy();
  });
});
