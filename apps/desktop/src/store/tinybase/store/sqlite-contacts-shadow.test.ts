import { describe, expect, it } from "vitest";

import {
  normalizeHumanRow,
  normalizeOrganizationRow,
} from "./sqlite-contacts-shadow";

describe("SQLite contact row normalization", () => {
  it("preserves all editable human fields", () => {
    expect(
      normalizeHumanRow({
        user_id: "user-1",
        created_at: "created",
        name: "Ada",
        email: "ada@example.com",
        phone: "+1",
        org_id: "org-1",
        job_title: "Engineer",
        linkedin_username: "ada",
        memo: "Met at launch",
        pinned: true,
        pin_order: 2,
      }),
    ).toEqual({
      user_id: "user-1",
      created_at: "created",
      name: "Ada",
      email: "ada@example.com",
      phone: "+1",
      org_id: "org-1",
      job_title: "Engineer",
      linkedin_username: "ada",
      memo: "Met at launch",
      pinned: true,
      pin_order: 2,
    });
  });

  it("keeps organization pin state stable", () => {
    expect(
      normalizeOrganizationRow({
        user_id: "user-1",
        created_at: "created",
        name: "Acme",
        pinned: true,
        pin_order: 4,
      }),
    ).toMatchObject({ name: "Acme", pinned: true, pin_order: 4 });
  });
});
