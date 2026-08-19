import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BrandLoadingView } from "./brand-loading-view";

describe("BrandLoadingView", () => {
  afterEach(cleanup);

  it("shows a loading status without a mark", () => {
    render(<BrandLoadingView />);

    const status = screen.getByRole("status", { name: "Loading" });
    expect(status.querySelector("svg")).toBeNull();
  });
});
