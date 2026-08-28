import * as stylex from "@stylexjs/stylex";
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

    expect(markup).toContain('data-slot="ai-icon"');
    expect(markup).toContain('data-slot="ai-icon-art"');
  });

  test("keeps letter-badge backgrounds when provided", () => {
    const defaultMarkup = renderToStaticMarkup(<AiIconSlot>G</AiIconSlot>);
    const markup = renderToStaticMarkup(
      <AiIconSlot sx={styles.letterBadge}>G</AiIconSlot>,
    );

    expect(markup).not.toBe(defaultMarkup);
  });
});

const styles = stylex.create({
  letterBadge: {
    backgroundColor: "rgb(255 251 235)",
    borderRadius: "0.375rem",
  },
});
