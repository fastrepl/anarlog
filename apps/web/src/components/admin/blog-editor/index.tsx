import "./blog-editor.css";

import { Markdown } from "@tiptap/markdown";
import {
  EditorContent,
  type Editor as TiptapEditor,
  useEditor,
} from "@tiptap/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useDebounceCallback } from "usehooks-ts";

import { getExtensions, type ImageUploadResult } from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";

import { ClipNode } from "./clip-embed";
import { GoogleDocsImport } from "./google-docs-import";
import { BlogImage } from "./image-with-alt";
import { Toolbar } from "./toolbar";

export type { TiptapEditor };

interface UseBlogEditorOptions {
  content?: string;
  editable?: boolean;
  onUpdate?: (markdown: string) => void;
  onImageUpload?: (file: File) => Promise<ImageUploadResult>;
}

export function useBlogEditor({
  content = "",
  editable = true,
  onUpdate,
  onImageUpload,
}: UseBlogEditorOptions) {
  const onEditorUpdate = useDebounceCallback(
    ({ editor }: { editor: TiptapEditor }) => {
      if (!editor.isInitialized || !onUpdate) {
        return;
      }
      const json = editor.getJSON();
      const markdown = editor.markdown?.serialize(json);
      if (markdown) {
        onUpdate(markdown);
      }
    },
    300,
  );

  const extensions = useMemo(
    () => [
      ...getExtensions(
        ({ node }) => {
          if (node.type.name === "paragraph") {
            return "Start typing...";
          }
          return "";
        },
        onImageUpload
          ? {
              onImageUpload,
            }
          : undefined,
        { imageExtension: BlogImage },
      ).map((ext) =>
        ext.name === "underline"
          ? ext.extend({
              renderMarkdown(
                _node: Record<string, unknown>,
                helpers: {
                  renderChildren: (node: Record<string, unknown>) => string;
                },
              ) {
                return helpers.renderChildren(_node);
              },
            })
          : ext,
      ),
      Markdown,
      ClipNode,
    ],
    [onImageUpload],
  );

  const editor = useEditor(
    {
      extensions,
      editable,
      content,
      contentType: "markdown",
      onCreate: ({ editor }) => {
        editor.view.dom.setAttribute("spellcheck", "false");
      },
      onUpdate: onEditorUpdate,
      immediatelyRender: false,
      shouldRerenderOnTransaction: false,
    },
    [extensions],
  );

  return editor;
}

interface BlogEditorProps {
  editor: TiptapEditor | null;
  editable?: boolean;
  showToolbar?: boolean;
  onGoogleDocsImport?: (url: string) => void;
  isImporting?: boolean;
  onAddImageFromLibrary?: () => void;
}

function BlogEditor({
  editor,
  editable = true,
  showToolbar = true,
  onGoogleDocsImport,
  isImporting,
  onAddImageFromLibrary,
}: BlogEditorProps) {
  const [showSearch, setShowSearch] = useState(false);
  const [showReplace, setShowReplace] = useState(false);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;

      if (isMod && e.key === "f") {
        e.preventDefault();
        setShowSearch((prev) => !prev);
        if (!showSearch) {
          setShowReplace(false);
        }
      }

      if (isMod && e.shiftKey && e.key === "h") {
        e.preventDefault();
        if (showSearch) {
          setShowReplace((prev) => !prev);
        } else {
          setShowSearch(true);
          setShowReplace(true);
        }
      }
    },
    [showSearch],
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  const showImportOverlay = editor?.isEmpty && onGoogleDocsImport && editable;

  useEffect(() => {
    const platform = navigator.platform.toLowerCase();
    if (platform.includes("win")) {
      document.body.classList.add("platform-windows");
    } else if (platform.includes("linux")) {
      document.body.classList.add("platform-linux");
    }

    return () => {
      document.body.classList.remove("platform-windows", "platform-linux");
    };
  }, []);

  return (
    <div className="relative flex h-full flex-col">
      {editable && showToolbar && (
        <div className="shrink-0">
          <Toolbar
            editor={editor}
            onAddImage={onAddImageFromLibrary}
            showSearch={showSearch}
            onShowSearchChange={setShowSearch}
            showReplace={showReplace}
            onShowReplaceChange={setShowReplace}
          />
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        <EditorContent
          editor={editor}
          className="tiptap-root blog-editor"
          role="textbox"
        />
        {showImportOverlay && (
          <div className="mt-6">
            <GoogleDocsImport
              onImport={onGoogleDocsImport}
              isLoading={isImporting}
            />
          </div>
        )}
      </div>
    </div>
  );
}

BlogEditor.displayName = "BlogEditor";

export default BlogEditor;
