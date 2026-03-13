import { describe, expect, test } from "vitest";

import { frontmatterToHuman, humanToFrontmatter } from "./transform";

describe("frontmatterToHuman", () => {
  test("converts emails array to comma-separated string", () => {
    const result = frontmatterToHuman(
      { emails: ["a@example.com", "b@example.com"] },
      "",
    );
    expect(result.email).toBe("a@example.com,b@example.com");
  });

  test("falls back to email string for backward compat", () => {
    const result = frontmatterToHuman({ email: "a@example.com" }, "");
    expect(result.email).toBe("a@example.com");
  });

  test("prefers emails array over email string", () => {
    const result = frontmatterToHuman(
      {
        emails: ["new@example.com"],
        email: "old@example.com",
      },
      "",
    );
    expect(result.email).toBe("new@example.com");
  });

  test("returns empty string when neither exists", () => {
    const result = frontmatterToHuman({}, "");
    expect(result.email).toBe("");
  });

  test("trims whitespace and filters empty values", () => {
    const result = frontmatterToHuman(
      { emails: ["  a@example.com  ", "", "  b@example.com"] },
      "",
    );
    expect(result.email).toBe("a@example.com,b@example.com");
  });

  test("includes body as memo", () => {
    const result = frontmatterToHuman({ name: "John" }, "Some notes here");
    expect(result.memo).toBe("Some notes here");
  });

  test("converts all frontmatter fields", () => {
    const result = frontmatterToHuman(
      {
        user_id: "user-1",
        name: "John Doe",
        emails: ["john@example.com"],
        org_id: "org-1",
        job_title: "Engineer",
        linkedin_username: "johndoe",
      },
      "Notes",
    );
    expect(result).toEqual({
      user_id: "user-1",
      created_at: undefined,
      name: "John Doe",
      email: "john@example.com",
      org_id: "org-1",
      job_title: "Engineer",
      linkedin_username: "johndoe",
      memo: "Notes",
      pinned: false,
      pin_order: undefined,
    });
  });
});

describe("humanToFrontmatter", () => {
  test("splits comma-separated string into array", () => {
    const result = humanToFrontmatter({
      user_id: "",
      name: "",
      email: "a@example.com,b@example.com",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
      created_at: "",
    });
    expect(result.frontmatter.emails).toEqual([
      "a@example.com",
      "b@example.com",
    ]);
  });

  test("returns empty array for empty string", () => {
    const result = humanToFrontmatter({
      user_id: "",
      name: "",
      email: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
      created_at: "",
    });
    expect(result.frontmatter.emails).toEqual([]);
  });

  test("trims whitespace and filters empty values", () => {
    const result = humanToFrontmatter({
      user_id: "",
      name: "",
      email: "  a@example.com  , , b@example.com  ",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
      created_at: "",
    });
    expect(result.frontmatter.emails).toEqual([
      "a@example.com",
      "b@example.com",
    ]);
  });

  test("handles single email", () => {
    const result = humanToFrontmatter({
      user_id: "",
      name: "",
      email: "a@example.com",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
      created_at: "",
    });
    expect(result.frontmatter.emails).toEqual(["a@example.com"]);
  });

  test("extracts memo as body", () => {
    const result = humanToFrontmatter({
      user_id: "",
      name: "",
      email: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "Some notes",
      pinned: false,
      created_at: "",
    });
    expect(result.body).toBe("Some notes");
  });

  test("converts all fields correctly", () => {
    const result = humanToFrontmatter({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      name: "John Doe",
      email: "john@example.com",
      org_id: "org-1",
      job_title: "Engineer",
      linkedin_username: "johndoe",
      memo: "Notes",
      pinned: false,
    });
    expect(result).toEqual({
      frontmatter: {
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        name: "John Doe",
        emails: ["john@example.com"],
        org_id: "org-1",
        job_title: "Engineer",
        linkedin_username: "johndoe",
        pinned: false,
        pin_order: 0,
      },
      body: "Notes",
    });
  });
});
