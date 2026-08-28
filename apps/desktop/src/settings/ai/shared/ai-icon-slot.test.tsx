import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { AiIconSlot, ProviderIconSlot } from "./index";

describe("provider icon slot", () => {
  test("renders product marks without gray tiles", () => {
    const markup = renderToStaticMarkup(
      <ProviderIconSlot>
        <svg />
      </ProviderIconSlot>,
    );

    expect(markup).not.toContain("bg-muted");
    expect(markup).not.toContain("rounded-md");
    expect(markup).toContain("size-5");
    expect(markup).toContain("size-full");
  });

  test("keeps letter-badge backgrounds when provided", () => {
    const markup = renderToStaticMarkup(
      <AiIconSlot className="rounded-md bg-amber-50">G</AiIconSlot>,
    );

    expect(markup).toContain("bg-amber-50");
    expect(markup).toContain("rounded-md");
  });
});
