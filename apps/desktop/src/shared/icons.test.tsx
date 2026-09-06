import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import * as icons from "@anlg/ui/components/icons";

describe("outline icons", () => {
  it.each(Object.entries(icons))(
    "renders %s without filled shapes at every supported weight",
    (_, Icon) => {
      for (const weight of ["thin", "light", "regular", "bold"] as const) {
        const { container, unmount } = render(
          <Icon size={16} weight={weight} />,
        );
        const svg = container.querySelector("svg");

        expect(svg?.getAttribute("fill")).toBe("none");
        expect(svg?.querySelector('[fill]:not([fill="none"])')).toBeNull();
        expect(svg?.querySelector('[stroke="currentColor"]')).not.toBeNull();
        unmount();
      }
    },
  );

  it("ignores legacy fill props forwarded through a spread", () => {
    const { container } = render(
      <icons.Play {...{ size: 16, fill: "currentColor" }} />,
    );

    expect(container.querySelector("svg")?.getAttribute("fill")).toBe("none");
  });
});
