import { describe, expect, it } from "vitest";

import { SCHEMA } from "@hypr/store";

import { SQLITE_SHADOWED_TABLES } from "./sqlite-shadow-cutover";

describe("SQLite shadow cutover coverage", () => {
  it("covers every table in the legacy main store", () => {
    expect([...SQLITE_SHADOWED_TABLES].sort()).toEqual(
      Object.keys(SCHEMA.table).sort(),
    );
  });
});
