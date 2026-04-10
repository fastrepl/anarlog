import { describe, expect, test, vi } from "vitest";

import {
  buildChatPath,
  buildEntityFilePath,
  buildEntityPath,
  buildSessionPath,
  createMarkdownEntityParser,
  getParentFolderPath,
  sanitizeFilename,
} from "./paths";

vi.mock("@tauri-apps/api/path", () => ({
  sep: vi.fn().mockReturnValue("/"),
}));

describe("buildSessionPath", () => {
  const dataDir = "/data/char";

  test("builds path without folder", () => {
    expect(buildSessionPath(dataDir, "session-123")).toBe(
      "/data/char/sessions/session-123",
    );
  });

  test("builds path with empty folder", () => {
    expect(buildSessionPath(dataDir, "session-123", "")).toBe(
      "/data/char/sessions/session-123",
    );
  });

  test("builds path with single-level folder", () => {
    expect(buildSessionPath(dataDir, "session-123", "work")).toBe(
      "/data/char/sessions/work/session-123",
    );
  });

  test("builds path with nested folder", () => {
    expect(buildSessionPath(dataDir, "session-123", "work/project-a")).toBe(
      "/data/char/sessions/work/project-a/session-123",
    );
  });

  test("builds path with deeply nested folder", () => {
    expect(
      buildSessionPath(dataDir, "session-123", "work/project-a/meetings"),
    ).toBe("/data/char/sessions/work/project-a/meetings/session-123");
  });
});

describe("buildChatPath", () => {
  const dataDir = "/data/char";

  test("builds chat path", () => {
    expect(buildChatPath(dataDir, "chat-456")).toBe(
      "/data/char/chats/chat-456",
    );
  });

  test("builds chat path with uuid", () => {
    expect(buildChatPath(dataDir, "550e8400-e29b-41d4-a716-446655440000")).toBe(
      "/data/char/chats/550e8400-e29b-41d4-a716-446655440000",
    );
  });
});

describe("buildEntityPath", () => {
  const dataDir = "/data/char";

  test("builds entity path for humans", () => {
    expect(buildEntityPath(dataDir, "humans")).toBe("/data/char/humans");
  });

  test("builds entity path for organizations", () => {
    expect(buildEntityPath(dataDir, "organizations")).toBe(
      "/data/char/organizations",
    );
  });

  test("builds entity path for prompts", () => {
    expect(buildEntityPath(dataDir, "prompts")).toBe("/data/char/prompts");
  });
});

describe("buildEntityFilePath", () => {
  const dataDir = "/data/char";

  test("builds entity file path with .md extension", () => {
    expect(buildEntityFilePath(dataDir, "humans", "person-123")).toBe(
      "/data/char/humans/person-123.md",
    );
  });

  test("builds entity file path for uuid", () => {
    expect(
      buildEntityFilePath(
        dataDir,
        "humans",
        "550e8400-e29b-41d4-a716-446655440000",
      ),
    ).toBe("/data/char/humans/550e8400-e29b-41d4-a716-446655440000.md");
  });

  test("builds entity file path for organizations", () => {
    expect(buildEntityFilePath(dataDir, "organizations", "acme-corp")).toBe(
      "/data/char/organizations/acme-corp.md",
    );
  });
});

describe("getParentFolderPath", () => {
  test("returns empty string for empty input", () => {
    expect(getParentFolderPath("")).toBe("");
  });

  test("returns empty string for single-level folder", () => {
    expect(getParentFolderPath("work")).toBe("");
  });

  test("returns parent for two-level path", () => {
    expect(getParentFolderPath("work/projects")).toBe("work");
  });

  test("returns parent for three-level path", () => {
    expect(getParentFolderPath("work/projects/frontend")).toBe("work/projects");
  });

  test("handles path with trailing content", () => {
    expect(getParentFolderPath("a/b/c/d")).toBe("a/b/c");
  });
});

describe("sanitizeFilename", () => {
  test("returns unchanged for valid filename", () => {
    expect(sanitizeFilename("valid-filename")).toBe("valid-filename");
  });

  test("replaces less than sign", () => {
    expect(sanitizeFilename("file<name")).toBe("file_name");
  });

  test("replaces greater than sign", () => {
    expect(sanitizeFilename("file>name")).toBe("file_name");
  });

  test("replaces colon", () => {
    expect(sanitizeFilename("file:name")).toBe("file_name");
  });

  test("replaces double quote", () => {
    expect(sanitizeFilename('file"name')).toBe("file_name");
  });

  test("replaces forward slash", () => {
    expect(sanitizeFilename("file/name")).toBe("file_name");
  });

  test("replaces backslash", () => {
    expect(sanitizeFilename("file\\name")).toBe("file_name");
  });

  test("replaces pipe", () => {
    expect(sanitizeFilename("file|name")).toBe("file_name");
  });

  test("replaces question mark", () => {
    expect(sanitizeFilename("file?name")).toBe("file_name");
  });

  test("replaces asterisk", () => {
    expect(sanitizeFilename("file*name")).toBe("file_name");
  });

  test("replaces multiple special characters", () => {
    expect(sanitizeFilename('file<>:"/\\|?*name')).toBe("file_________name");
  });

  test("trims whitespace", () => {
    expect(sanitizeFilename("  filename  ")).toBe("filename");
  });

  test("handles complex case", () => {
    expect(sanitizeFilename('  Meeting: "Project Review" Q&A?  ')).toBe(
      "Meeting_ _Project Review_ Q&A_",
    );
  });

  test("preserves dots and dashes", () => {
    expect(sanitizeFilename("file-name.test")).toBe("file-name.test");
  });

  test("preserves underscores", () => {
    expect(sanitizeFilename("file_name_test")).toBe("file_name_test");
  });
});

describe("createMarkdownEntityParser", () => {
  const parseHumanId = createMarkdownEntityParser("humans");

  describe("relative paths (from notify events)", () => {
    test("parses id from valid path", () => {
      expect(parseHumanId("humans/person-123.md")).toBe("person-123");
    });

    test("parses uuid from path", () => {
      expect(
        parseHumanId("humans/550e8400-e29b-41d4-a716-446655440000.md"),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });

    test("parses id with special characters", () => {
      expect(parseHumanId("humans/john-doe_2024.md")).toBe("john-doe_2024");
    });
  });

  describe("edge cases", () => {
    test("returns null for non-markdown file", () => {
      expect(parseHumanId("humans/person-123.json")).toBeNull();
    });

    test("returns null for wrong directory", () => {
      expect(parseHumanId("organizations/org-123.md")).toBeNull();
    });

    test("returns null for path without filename", () => {
      expect(parseHumanId("humans/")).toBeNull();
    });

    test("returns null for directory name only", () => {
      expect(parseHumanId("humans")).toBeNull();
    });

    test("returns null for empty path", () => {
      expect(parseHumanId("")).toBeNull();
    });
  });

  describe("absolute paths (defensive handling)", () => {
    test("parses id from path with leading segments", () => {
      expect(parseHumanId("/data/char/humans/person-123.md")).toBe(
        "person-123",
      );
    });
  });

  describe("different directory names", () => {
    test("works with organizations directory", () => {
      const parseOrgId = createMarkdownEntityParser("organizations");
      expect(parseOrgId("organizations/acme-corp.md")).toBe("acme-corp");
      expect(parseOrgId("humans/person.md")).toBeNull();
    });

    test("works with prompts directory", () => {
      const parsePromptId = createMarkdownEntityParser("prompts");
      expect(parsePromptId("prompts/my-prompt.md")).toBe("my-prompt");
    });
  });
});
