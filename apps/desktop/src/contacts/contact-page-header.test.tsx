import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ContactPageHeader } from "./contact-page-header";

afterEach(cleanup);

describe("ContactPageHeader", () => {
  it("adds the compact identity beside the title when requested", () => {
    const props = {
      title: "Ada Lovelace",
      compactIdentity: <span>Contact avatar</span>,
      pinned: false,
      onTogglePin: vi.fn(),
      onDelete: vi.fn(),
    };
    const { rerender } = render(
      <ContactPageHeader {...props} showCompactIdentity={false} />,
    );

    expect(
      screen.getByRole("heading", { name: "Ada Lovelace" }),
    ).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Contact options" }),
    ).not.toBeNull();
    expect(screen.queryByText("Contact avatar")).toBeNull();

    rerender(<ContactPageHeader {...props} showCompactIdentity={true} />);
    expect(screen.getByText("Contact avatar")).not.toBeNull();
  });
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
