import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { ContactPageHeader } from "./contact-page-header";

afterEach(cleanup);

it("renders toolbar actions unless read-only", () => {
  const props = {
    title: "Ada Lovelace",
    compactIdentity: null,
    pinned: false,
    onTogglePin: vi.fn(),
    onDelete: vi.fn(),
    actions: <button>Enrich</button>,
  };
  const { rerender } = render(
    <ContactPageHeader {...props} showCompactIdentity={false} />,
  );
  expect(screen.getByRole("button", { name: "Enrich" })).not.toBeNull();

  rerender(
    <ContactPageHeader {...props} showCompactIdentity={false} readOnly />,
  );
  expect(screen.queryByRole("button", { name: "Enrich" })).toBeNull();
});

it("hides mutation options for your own contact", () => {
  render(
    <ContactPageHeader
      title="Me"
      compactIdentity={null}
      showCompactIdentity={false}
      pinned
      readOnly
      onTogglePin={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
  expect(screen.queryByRole("button", { name: "Contact options" })).toBeNull();
  expect(screen.getByRole("heading", { name: "Me" })).not.toBeNull();
});
