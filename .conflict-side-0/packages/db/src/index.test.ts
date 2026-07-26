import { beforeEach, describe, expect, it, vi } from "vitest";

import { createDb, max, templates } from "./index";

describe("@hypr/db createDb", () => {
  const executeProxy = vi.fn();
  const db = createDb({ executeProxy });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("uses executeProxy for inserts", async () => {
    executeProxy.mockResolvedValue({ rows: [] });

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

    expect(executeProxy).toHaveBeenCalledWith(
      expect.stringContaining('insert into "templates"'),
      expect.any(Array),
      "run",
    );
  });

  it("maps proxy rows for findMany", async () => {
    executeProxy.mockResolvedValue({
      rows: [
        [
          "template-1",
          "One",
          "",
          0,
          null,
          null,
          null,
          "[]",
          "2026-04-14T00:00:00Z",
          "2026-04-14T00:00:00Z",
        ],
      ],
    });

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
    executeProxy.mockResolvedValue({
      rows: [
        "template-1",
        "One",
        "",
        0,
        null,
        null,
        null,
        "[]",
        "2026-04-14T00:00:00Z",
        "2026-04-14T00:00:00Z",
      ],
    });

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

  it("passes all mode through to the proxy client", async () => {
    executeProxy.mockResolvedValue({ rows: [] });

    await db.select().from(templates);

    expect(executeProxy).toHaveBeenCalledWith(
      expect.stringContaining('select "id", "title"'),
      expect.any(Array),
      "all",
    );
  });

  it("maps aggregate query rows from the proxy client", async () => {
    executeProxy.mockResolvedValue({ rows: [[7]] });

    await expect(
      db.select({ maxOrder: max(templates.pinOrder) }).from(templates),
    ).resolves.toEqual([{ maxOrder: 7 }]);
  });

  it("logs proxy errors and rethrows", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const error = new Error("proxy failed");
    executeProxy.mockRejectedValue(error);

    await expect(db.select().from(templates)).rejects.toThrow(/Failed query:/);
    expect(errorSpy).toHaveBeenCalledWith(
      "[drizzle-proxy]",
      "all",
      expect.stringContaining('select "id", "title"'),
      error,
    );
    errorSpy.mockRestore();
  });
});
