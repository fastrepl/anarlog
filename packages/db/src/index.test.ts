import { beforeEach, describe, expect, it, vi } from "vitest";

import { createDb, templates } from "./index";

describe("@hypr/db createDb", () => {
  const execute = vi.fn();
  const db = createDb({ execute });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("uses execute for inserts", async () => {
    execute.mockResolvedValue([]);

    await db.insert(templates).values({
      id: "template-1",
      title: "New Template",
      description: "",
      pinned: false,
      pinOrder: null,
      category: null,
      targetsJson: null,
      sectionsJson: [],
      createdAt: "2026-04-14T00:00:00Z",
      updatedAt: "2026-04-14T00:00:00Z",
    });

    expect(execute).toHaveBeenCalledWith(
      expect.stringContaining('insert into "templates"'),
      expect.any(Array),
    );
  });

  it("maps query rows for findMany", async () => {
    execute.mockResolvedValue([
      {
        id: "template-1",
        title: "One",
        description: "",
        pinned: 0,
        pin_order: null,
        category: null,
        targets_json: null,
        sections_json: "[]",
        created_at: "2026-04-14T00:00:00Z",
        updated_at: "2026-04-14T00:00:00Z",
      },
    ]);

    await expect(db.select().from(templates)).resolves.toEqual([
      {
        id: "template-1",
        title: "One",
        description: "",
        pinned: false,
        pinOrder: null,
        category: null,
        targetsJson: null,
        sectionsJson: [],
        createdAt: "2026-04-14T00:00:00Z",
        updatedAt: "2026-04-14T00:00:00Z",
      },
    ]);
  });

  it("uses get mode for findFirst", async () => {
    execute.mockResolvedValue([
      {
        id: "template-1",
        title: "One",
        description: "",
        pinned: 0,
        pin_order: null,
        category: null,
        targets_json: null,
        sections_json: "[]",
        created_at: "2026-04-14T00:00:00Z",
        updated_at: "2026-04-14T00:00:00Z",
      },
    ]);

    await expect(db.query.templates.findFirst()).resolves.toEqual({
      id: "template-1",
      title: "One",
      description: "",
      pinned: false,
      pinOrder: null,
      category: null,
      targetsJson: null,
      sectionsJson: [],
      createdAt: "2026-04-14T00:00:00Z",
      updatedAt: "2026-04-14T00:00:00Z",
    });
  });
});
