import { afterEach, describe, expect, it } from "vitest";

import {
  decodeNotesView,
  encodeNotesView,
  folderIdForNewNote,
  resetSidebarNotes,
  useSidebarNotes,
} from "./note-filter";

describe("sidebar note filter", () => {
  afterEach(() => {
    resetSidebarNotes();
  });

  it("encodes and decodes mine, unfiled, and named folders", () => {
    expect(encodeNotesView(null)).toBe("mine");
    expect(encodeNotesView("")).toBe("folder:");
    expect(encodeNotesView("CS 101")).toBe("folder:CS 101");

    expect(decodeNotesView("mine")).toEqual({
      noteFilter: "mine",
      folderFilter: null,
    });
    expect(decodeNotesView("shared")).toEqual({
      noteFilter: "mine",
      folderFilter: null,
    });
    expect(decodeNotesView("folder:")).toEqual({
      noteFilter: "mine",
      folderFilter: "",
    });
    expect(decodeNotesView("folder:CS 101")).toEqual({
      noteFilter: "mine",
      folderFilter: "CS 101",
    });
  });

  it("inherits the active folder for a new note", () => {
    expect(folderIdForNewNote(null)).toBeUndefined();
    expect(folderIdForNewNote("")).toBe("");
    expect(folderIdForNewNote("CS 101")).toBe("CS 101");
  });

  it("keeps grouping and sort independent of the ownership filter", () => {
    useSidebarNotes.getState().setGroupBy("folder");
    useSidebarNotes.getState().setSortOrder("oldest");
    useSidebarNotes.getState().setView("mine");

    expect(useSidebarNotes.getState()).toMatchObject({
      noteFilter: "mine",
      folderFilter: null,
      groupBy: "folder",
      sortOrder: "oldest",
    });
  });
});
