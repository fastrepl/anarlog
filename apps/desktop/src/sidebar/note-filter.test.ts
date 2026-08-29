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

  it("encodes and decodes mine, shared, unfiled, and named folders", () => {
    expect(encodeNotesView("mine", null)).toBe("mine");
    expect(encodeNotesView("shared", null)).toBe("shared");
    expect(encodeNotesView("mine", "")).toBe("folder:");
    expect(encodeNotesView("mine", "CS 101")).toBe("folder:CS 101");

    expect(decodeNotesView("mine")).toEqual({
      noteFilter: "mine",
      folderFilter: null,
    });
    expect(decodeNotesView("shared")).toEqual({
      noteFilter: "shared",
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

  it("inherits a folder only while that folder is the active mine view", () => {
    expect(folderIdForNewNote("mine", null)).toBeUndefined();
    expect(folderIdForNewNote("shared", "CS 101")).toBeUndefined();
    expect(folderIdForNewNote("mine", "")).toBe("");
    expect(folderIdForNewNote("mine", "CS 101")).toBe("CS 101");
  });

  it("clears the folder filter when switching to shared notes", () => {
    useSidebarNotes.getState().setView("mine", "CS 101");
    useSidebarNotes.getState().setView("shared");

    expect(useSidebarNotes.getState()).toMatchObject({
      noteFilter: "shared",
      folderFilter: null,
    });
  });
});
