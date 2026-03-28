import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { NormalMessage } from "./normal";

describe("NormalMessage", () => {
  test("renders markdown in user messages", async () => {
    render(
      <NormalMessage
        message={{
          id: "message-1",
          role: "user",
          parts: [{ type: "text", text: "[Docs](https://example.com/docs)" }],
        }}
      />,
    );

    const link = await screen.findByRole("link", {
      name: "Docs",
    });

    expect(link.getAttribute("href")).toBe("https://example.com/docs");
  });
});
