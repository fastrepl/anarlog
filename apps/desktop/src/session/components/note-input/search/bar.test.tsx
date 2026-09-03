import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  setSearch: vi.fn(),
  platform: vi.fn(() => "macos"),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: hoisted.platform,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: React.ReactNode }) => children,
  useLingui: () => ({
    t: (parts: TemplateStringsArray) => parts.join(""),
  }),
}));

vi.mock("@anlg/ui/components/ui/kbd", () => ({
  Kbd: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("@anlg/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("./context", () => ({
  useSearch: () => ({
    query: "common",
    currentMatchIndex: 0,
    totalMatches: 1,
    onNext: vi.fn(),
    onPrev: vi.fn(),
    caseSensitive: false,
    wholeWord: false,
    showReplace: false,
    replaceQuery: "",
    close: vi.fn(),
    setQuery: vi.fn(),
    toggleCaseSensitive: vi.fn(),
    toggleWholeWord: vi.fn(),
    toggleReplace: vi.fn(),
    setReplaceQuery: vi.fn(),
  }),
}));

import { SearchBar } from "./bar";

afterEach(() => {
  hoisted.setSearch.mockClear();
  hoisted.platform.mockReturnValue("macos");
});

describe("SearchBar", () => {
  it("clears editor highlights whenever the search UI closes", () => {
    const editorRef = {
      current: {
        commands: {
          setSearch: hoisted.setSearch,
        },
      },
    } as any;

    const { unmount } = render(<SearchBar editorRef={editorRef} />);
    unmount();

    expect(hoisted.setSearch).toHaveBeenCalledOnce();
    expect(hoisted.setSearch).toHaveBeenCalledWith("", false);
  });

  it("shows the Windows shortcut modifier", () => {
    hoisted.platform.mockReturnValue("windows");
    const editorRef = {
      current: {
        commands: {
          setSearch: hoisted.setSearch,
        },
      },
    } as any;

    const { container } = render(<SearchBar editorRef={editorRef} />);

    expect(container.textContent).toContain("Ctrl H");
  });

  it("hides replace controls for find-only surfaces", () => {
    const { container } = render(<SearchBar allowReplace={false} />);

    expect(container.textContent).not.toContain("Replace");
    expect(container.textContent).not.toContain("⌘ H");
  });
});
