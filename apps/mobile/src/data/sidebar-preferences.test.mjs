import assert from "node:assert/strict";
import test from "node:test";

import { parseSidebarPreferences } from "./sidebar-preferences-model.ts";

test("uses the desktop notes-list defaults", () => {
  assert.deepEqual(parseSidebarPreferences([]), {
    showFolder: true,
    showTags: false,
  });
});

test("prefers synced notes-list settings over device values", () => {
  assert.deepEqual(
    parseSidebarPreferences([
      {
        id: "sidebar_show_folder",
        value_json: "false",
        source_rank: 0,
      },
      {
        id: "sidebar_show_folder",
        value_json: "true",
        source_rank: 1,
      },
      {
        id: "sidebar_show_tags",
        value_json: "true",
        source_rank: 1,
      },
    ]),
    { showFolder: true, showTags: true },
  );
});

test("ignores malformed notes-list settings", () => {
  assert.deepEqual(
    parseSidebarPreferences([
      {
        id: "sidebar_show_folder",
        value_json: '"false"',
        source_rank: 1,
      },
      {
        id: "sidebar_show_tags",
        value_json: "not-json",
        source_rank: 1,
      },
    ]),
    { showFolder: true, showTags: false },
  );
});
