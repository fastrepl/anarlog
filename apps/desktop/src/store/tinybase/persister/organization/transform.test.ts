import { describe, expect, test } from "vitest";

import {
  frontmatterToOrganization,
  organizationToFrontmatter,
} from "./transform";

describe("frontmatterToOrganization", () => {
  test("converts frontmatter to organization storage", () => {
    const result = frontmatterToOrganization(
      {
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        name: "Acme Corp",
      },
      "",
    );
    expect(result).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      name: "Acme Corp",
      pinned: false,
      pin_order: undefined,
    });
  });

  test("handles missing fields", () => {
    const result = frontmatterToOrganization({}, "");
    expect(result).toEqual({
      user_id: "",
      created_at: undefined,
      name: "",
      pinned: false,
      pin_order: undefined,
    });
  });

  test("preserves pinned state", () => {
    const result = frontmatterToOrganization(
      {
        user_id: "user-1",
        name: "Acme Corp",
        pinned: true,
      },
      "",
    );
    expect(result).toEqual({
      user_id: "user-1",
      created_at: undefined,
      name: "Acme Corp",
      pinned: true,
      pin_order: undefined,
    });
  });
});

describe("organizationToFrontmatter", () => {
  test("converts organization storage to frontmatter", () => {
    const result = organizationToFrontmatter({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      name: "Acme Corp",
      pinned: false,
    });
    expect(result).toEqual({
      frontmatter: {
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        name: "Acme Corp",
        pinned: false,
        pin_order: 0,
      },
      body: "",
    });
  });

  test("converts pinned organization to frontmatter", () => {
    const result = organizationToFrontmatter({
      user_id: "user-1",
      name: "Acme Corp",
      pinned: true,
    });
    expect(result).toEqual({
      frontmatter: {
        user_id: "user-1",
        created_at: "",
        name: "Acme Corp",
        pinned: true,
        pin_order: 0,
      },
      body: "",
    });
  });
});
