import { type Editor as TiptapEditor, useEditorState } from "@tiptap/react";
import {
  BoldIcon,
  CaseSensitiveIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CodeIcon,
  FilmIcon,
  Heading1Icon,
  Heading2Icon,
  Heading3Icon,
  HighlighterIcon,
  ImageIcon,
  ItalicIcon,
  LinkIcon,
  ListIcon,
  ListOrderedIcon,
  QuoteIcon,
  ReplaceAllIcon,
  ReplaceIcon,
  SearchIcon,
  StrikethroughIcon,
  TableIcon,
  UnderlineIcon,
  XIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { parseYouTubeUrl } from "@hypr/tiptap/shared";

interface ToolbarProps {
  editor: TiptapEditor | null;
  onAddImage?: () => void;
  showSearch: boolean;
  onShowSearchChange: (show: boolean) => void;
  showReplace: boolean;
  onShowReplaceChange: (show: boolean) => void;
}

interface ToolbarButtonProps {
  onClick: () => void;
  isActive?: boolean;
  disabled?: boolean;
  title: string;
  children: React.ReactNode;
}

function ToolbarButton({
  onClick,
  isActive,
  disabled,
  title,
  children,
}: ToolbarButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={[
        "p-1.5 rounded transition-colors",
        isActive
          ? "bg-neutral-200 text-neutral-900"
          : "text-neutral-600 hover:bg-neutral-100 hover:text-neutral-900",
        disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function ToolbarDivider() {
  return <div className="mx-1 h-5 w-px bg-neutral-200" />;
}

export function Toolbar({
  editor,
  onAddImage,
  showSearch,
  onShowSearchChange,
  showReplace,
  onShowReplaceChange,
}: ToolbarProps) {
  const [showLinkInput, setShowLinkInput] = useState(false);
  const [linkUrl, setLinkUrl] = useState("");
  const [showClipInput, setShowClipInput] = useState(false);
  const [clipUrl, setClipUrl] = useState("");
  const [searchTerm, setSearchTerm] = useState("");
  const [replaceTerm, setReplaceTerm] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const replaceInputRef = useRef<HTMLInputElement>(null);

  const editorState = useEditorState({
    editor,
    selector: (ctx) => ({
      isBold: ctx.editor?.isActive("bold") ?? false,
      isItalic: ctx.editor?.isActive("italic") ?? false,
      isUnderline: ctx.editor?.isActive("underline") ?? false,
      isStrike: ctx.editor?.isActive("strike") ?? false,
      isHighlight: ctx.editor?.isActive("highlight") ?? false,
      isHeading2: ctx.editor?.isActive("heading", { level: 2 }) ?? false,
      isHeading3: ctx.editor?.isActive("heading", { level: 3 }) ?? false,
      isHeading4: ctx.editor?.isActive("heading", { level: 4 }) ?? false,
      isBulletList: ctx.editor?.isActive("bulletList") ?? false,
      isOrderedList: ctx.editor?.isActive("orderedList") ?? false,
      isBlockquote: ctx.editor?.isActive("blockquote") ?? false,
      isCodeBlock: ctx.editor?.isActive("codeBlock") ?? false,
      isLink: ctx.editor?.isActive("link") ?? false,
      searchResults: ctx.editor?.storage.searchAndReplace?.results ?? [],
      searchResultIndex: ctx.editor?.storage.searchAndReplace?.resultIndex ?? 0,
    }),
  });

  useEffect(() => {
    if (!editor) return;
    editor.commands.setSearchTerm(searchTerm);
  }, [editor, searchTerm]);

  useEffect(() => {
    if (!editor) return;
    editor.commands.setReplaceTerm(replaceTerm);
  }, [editor, replaceTerm]);

  useEffect(() => {
    if (!editor) return;
    editor.commands.setCaseSensitive(caseSensitive);
  }, [editor, caseSensitive]);

  useEffect(() => {
    if (!showSearch && editor) {
      setSearchTerm("");
      setReplaceTerm("");
      editor.commands.setSearchTerm("");
      editor.commands.setReplaceTerm("");
      editor.commands.resetIndex();
      // Force a transaction to clear decorations
      editor.view.dispatch(editor.state.tr);
    }
  }, [showSearch, editor]);

  useEffect(() => {
    if (showSearch && editor) {
      const { from, to } = editor.state.selection;
      if (from !== to) {
        const selectedText = editor.state.doc.textBetween(from, to, " ");
        if (selectedText.trim()) {
          setSearchTerm(selectedText);
        }
      }
      if (searchInputRef.current) {
        searchInputRef.current.focus();
        searchInputRef.current.select();
      }
    }
  }, [showSearch, editor]);

  useEffect(() => {
    if (showReplace && replaceInputRef.current) {
      replaceInputRef.current.focus();
    }
  }, [showReplace]);

  const handleLinkSubmit = useCallback(() => {
    if (!editor) return;

    if (!linkUrl || linkUrl === "") {
      editor.chain().focus().extendMarkRange("link").unsetLink().run();
    } else {
      editor
        .chain()
        .focus()
        .extendMarkRange("link")
        .setLink({ href: linkUrl })
        .run();
    }

    setLinkUrl("");
    setShowLinkInput(false);
  }, [editor, linkUrl]);

  const handleCloseSearch = useCallback(() => {
    onShowSearchChange(false);
    onShowReplaceChange(false);
  }, [onShowSearchChange, onShowReplaceChange]);

  if (!editor) {
    return null;
  }

  const resultCount = editorState?.searchResults.length ?? 0;
  const currentIndex = (editorState?.searchResultIndex ?? 0) + 1;

  return (
    <div className="border-b border-neutral-200 bg-white">
      <div className="flex flex-wrap items-center gap-0.5 p-2">
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBold().run()}
          isActive={editorState?.isBold}
          title="Bold (Cmd+B)"
        >
          <BoldIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleItalic().run()}
          isActive={editorState?.isItalic}
          title="Italic (Cmd+I)"
        >
          <ItalicIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleUnderline().run()}
          isActive={editorState?.isUnderline}
          title="Underline (Cmd+U)"
        >
          <UnderlineIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleStrike().run()}
          isActive={editorState?.isStrike}
          title="Strikethrough"
        >
          <StrikethroughIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleHighlight().run()}
          isActive={editorState?.isHighlight}
          title="Highlight (==text==)"
        >
          <HighlighterIcon className="size-4" />
        </ToolbarButton>

        <ToolbarDivider />

        <ToolbarButton
          onClick={() =>
            editor.chain().focus().toggleHeading({ level: 2 }).run()
          }
          isActive={editorState?.isHeading2}
          title="Heading 1 (##)"
        >
          <Heading1Icon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() =>
            editor.chain().focus().toggleHeading({ level: 3 }).run()
          }
          isActive={editorState?.isHeading3}
          title="Heading 2 (###)"
        >
          <Heading2Icon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() =>
            editor.chain().focus().toggleHeading({ level: 4 }).run()
          }
          isActive={editorState?.isHeading4}
          title="Heading 3 (####)"
        >
          <Heading3Icon className="size-4" />
        </ToolbarButton>

        <ToolbarDivider />

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBulletList().run()}
          isActive={editorState?.isBulletList}
          title="Bullet List"
        >
          <ListIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
          isActive={editorState?.isOrderedList}
          title="Numbered List"
        >
          <ListOrderedIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
          isActive={editorState?.isBlockquote}
          title="Quote"
        >
          <QuoteIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() => editor.chain().focus().toggleCodeBlock().run()}
          isActive={editorState?.isCodeBlock}
          title="Code Block"
        >
          <CodeIcon className="size-4" />
        </ToolbarButton>

        <ToolbarButton
          onClick={() =>
            editor
              .chain()
              .focus()
              .insertTable({ rows: 3, cols: 3, withHeaderRow: true })
              .run()
          }
          title="Insert Table"
        >
          <TableIcon className="size-4" />
        </ToolbarButton>

        <ToolbarDivider />

        <div className="relative">
          <ToolbarButton
            onClick={() => {
              if (editorState?.isLink) {
                editor.chain().focus().unsetLink().run();
              } else {
                setShowLinkInput(!showLinkInput);
              }
            }}
            isActive={editorState?.isLink}
            title="Link"
          >
            <LinkIcon className="size-4" />
          </ToolbarButton>

          {showLinkInput && (
            <div className="absolute top-full left-0 z-10 mt-1 flex gap-2 rounded border border-neutral-200 bg-white p-2 shadow-lg">
              <input
                type="url"
                value={linkUrl}
                onChange={(e) => setLinkUrl(e.target.value)}
                placeholder="https://..."
                className="w-48 rounded border border-neutral-200 px-2 py-1 text-sm focus:border-blue-500 focus:outline-none"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleLinkSubmit();
                  } else if (e.key === "Escape") {
                    setShowLinkInput(false);
                    setLinkUrl("");
                  }
                }}
                autoFocus
              />
              <button
                type="button"
                onClick={handleLinkSubmit}
                className="rounded bg-blue-600 px-2 py-1 text-sm text-white hover:bg-blue-700"
              >
                Add
              </button>
            </div>
          )}
        </div>

        {onAddImage && (
          <>
            <ToolbarDivider />
            <ToolbarButton
              onClick={onAddImage}
              title="Add Image from Media Library"
            >
              <ImageIcon className="size-4" />
            </ToolbarButton>
          </>
        )}

        <div className="relative">
          <ToolbarButton
            onClick={() => setShowClipInput(!showClipInput)}
            title="Insert Clip"
          >
            <FilmIcon className="size-4" />
          </ToolbarButton>

          {showClipInput && (
            <div className="absolute top-full left-0 z-10 mt-1 flex gap-2 rounded border border-neutral-200 bg-white p-2 shadow-lg">
              <input
                type="url"
                value={clipUrl}
                onChange={(e) => setClipUrl(e.target.value)}
                placeholder="YouTube embed URL..."
                className="w-64 rounded border border-neutral-200 px-2 py-1 text-sm focus:border-blue-500 focus:outline-none"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    if (editor && clipUrl.trim()) {
                      const parsed = parseYouTubeUrl(clipUrl.trim());
                      const embedSrc = parsed?.embedUrl ?? clipUrl.trim();
                      editor
                        .chain()
                        .focus()
                        .insertContent({
                          type: "clip",
                          attrs: { src: embedSrc },
                        })
                        .run();
                      setClipUrl("");
                      setShowClipInput(false);
                    }
                  } else if (e.key === "Escape") {
                    setShowClipInput(false);
                    setClipUrl("");
                  }
                }}
                autoFocus
              />
              <button
                type="button"
                onClick={() => {
                  if (editor && clipUrl.trim()) {
                    const parsed = parseYouTubeUrl(clipUrl.trim());
                    const embedSrc = parsed?.embedUrl ?? clipUrl.trim();
                    editor
                      .chain()
                      .focus()
                      .insertContent({
                        type: "clip",
                        attrs: { src: embedSrc },
                      })
                      .run();
                    setClipUrl("");
                    setShowClipInput(false);
                  }
                }}
                className="rounded bg-blue-600 px-2 py-1 text-sm text-white hover:bg-blue-700"
              >
                Add
              </button>
            </div>
          )}
        </div>

        <div className="flex-1" />

        <ToolbarButton
          onClick={() => onShowSearchChange(!showSearch)}
          isActive={showSearch}
          title="Search & Replace (Cmd+F)"
        >
          <SearchIcon className="size-4" />
        </ToolbarButton>
      </div>

      {showSearch && (
        <div className="space-y-2 px-2 pb-2">
          <div className="flex items-center gap-2">
            <div className="flex flex-1 items-center gap-1 rounded border border-neutral-200 px-2 py-1.5 focus-within:border-blue-500">
              <input
                ref={searchInputRef}
                type="text"
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                placeholder="Search..."
                className="min-w-0 flex-1 bg-transparent text-sm focus:outline-none"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    editor.commands.nextSearchResult();
                  } else if (e.key === "Escape") {
                    handleCloseSearch();
                  }
                }}
              />
            </div>

            <button
              type="button"
              onClick={() => setCaseSensitive(!caseSensitive)}
              title="Case Sensitive"
              className={[
                "p-1.5 rounded text-sm font-medium transition-colors",
                caseSensitive
                  ? "bg-neutral-200 text-neutral-900"
                  : "text-neutral-500 hover:bg-neutral-100",
              ].join(" ")}
            >
              <CaseSensitiveIcon className="size-4" />
            </button>

            <button
              type="button"
              onClick={() => onShowReplaceChange(!showReplace)}
              title="Toggle Replace (Cmd+Shift+H)"
              className={[
                "p-1.5 rounded text-sm transition-colors",
                showReplace
                  ? "bg-neutral-200 text-neutral-900"
                  : "text-neutral-500 hover:bg-neutral-100",
              ].join(" ")}
            >
              <ReplaceIcon className="size-4" />
            </button>

            <ToolbarDivider />

            <button
              type="button"
              onClick={() => editor.commands.previousSearchResult()}
              disabled={resultCount === 0}
              className="rounded p-1.5 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
              title="Previous (Shift+Enter)"
            >
              <ChevronLeftIcon className="size-4" />
            </button>

            <button
              type="button"
              onClick={() => editor.commands.nextSearchResult()}
              disabled={resultCount === 0}
              className="rounded p-1.5 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
              title="Next (Enter)"
            >
              <ChevronRightIcon className="size-4" />
            </button>

            <span className="min-w-[3rem] text-center text-xs text-neutral-500">
              {searchTerm
                ? resultCount > 0
                  ? `${currentIndex}/${resultCount}`
                  : "0/0"
                : ""}
            </span>

            <button
              type="button"
              onClick={handleCloseSearch}
              className="rounded p-1.5 hover:bg-neutral-100"
              title="Close (Escape)"
            >
              <XIcon className="size-4" />
            </button>
          </div>

          {showReplace && (
            <div className="flex items-center gap-2">
              <div className="flex flex-1 items-center gap-1 rounded border border-neutral-200 px-2 py-1.5 focus-within:border-blue-500">
                <input
                  ref={replaceInputRef}
                  type="text"
                  value={replaceTerm}
                  onChange={(e) => setReplaceTerm(e.target.value)}
                  placeholder="Replace with..."
                  className="min-w-0 flex-1 bg-transparent text-sm focus:outline-none"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      editor.commands.replace();
                    } else if (e.key === "Escape") {
                      handleCloseSearch();
                    }
                  }}
                />
              </div>

              <button
                type="button"
                onClick={() => editor.commands.replace()}
                disabled={resultCount === 0}
                title="Replace"
                className="rounded p-1.5 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <ReplaceIcon className="size-4" />
              </button>

              <button
                type="button"
                onClick={() => editor.commands.replaceAll()}
                disabled={resultCount === 0}
                title="Replace All"
                className="rounded p-1.5 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <ReplaceAllIcon className="size-4" />
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
