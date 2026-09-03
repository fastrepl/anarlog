import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signIn: vi.fn(() => Promise.resolve()),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ signIn: mocks.signIn }),
}));

import { BeforeLogin } from "./before-login";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("offers one account action", () => {
  render(<BeforeLogin />);

  expect(screen.getAllByRole("button")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Get started" }));

  expect(mocks.signIn).toHaveBeenCalledTimes(1);
});
