import "../../styles.css";

import { Extension } from "@tiptap/core";
import {
  EditorContent,
  type JSONContent,
  type Editor as TiptapEditor,
  useEditor,
} from "@tiptap/react";
import { forwardRef, useEffect, useMemo, useRef } from "react";

import {
  isMentionActive,
  mention,
  type MentionConfig,
} from "../editor/mention";
import * as shared from "../shared";
import type { PlaceholderFunction } from "../shared/extensions/placeholder";

export type { JSONContent, TiptapEditor };
export type { MentionConfig };

export interface SlashCommandConfig {
  handleSearch: (
    query: string,
  ) => Promise<{ id: string; type: string; label: string; content?: string }[]>;
}

interface ChatEditorProps {
  initialContent?: JSONContent;
  editable?: boolean;
  placeholderComponent?: PlaceholderFunction;
  slashCommandConfig?: SlashCommandConfig;
  onUpdate?: (json: JSONContent) => void;
  onSubmit?: () => void;
}

const ChatEditor = forwardRef<{ editor: TiptapEditor | null }, ChatEditorProps>(
  (
    {
      initialContent,
      editable = true,
      placeholderComponent,
      slashCommandConfig,
      onUpdate,
      onSubmit,
    },
    ref,
  ) => {
    const previousContentRef = useRef<JSONContent>(initialContent);
    const slashCommandConfigRef = useRef(slashCommandConfig);
    slashCommandConfigRef.current = slashCommandConfig;
    const onUpdateRef = useRef(onUpdate);
    onUpdateRef.current = onUpdate;
    const onSubmitRef = useRef(onSubmit);
    onSubmitRef.current = onSubmit;

    const mentionConfigs = useMemo(() => {
      const configs: MentionConfig[] = [];

      if (slashCommandConfigRef.current) {
        configs.push({
          trigger: "@",
          handleSearch: (query) =>
            slashCommandConfigRef.current!.handleSearch(query),
        });
      }

      return configs;
    }, []);

    const submitOnEnter = useMemo(
      () =>
        Extension.create({
          name: "submitOnEnter",
          addKeyboardShortcuts() {
            return {
              "Mod-Enter": ({ editor }) => {
                if (isMentionActive(editor.state)) {
                  return false;
                }
                onSubmitRef.current?.();
                return true;
              },
            };
          },
        }),
      [],
    );

    const extensions = useMemo(
      () => [
        ...shared.getExtensions(placeholderComponent),
        ...mentionConfigs.map((config) => mention(config)),
        submitOnEnter,
      ],
      [mentionConfigs, placeholderComponent, submitOnEnter],
    );

    const editor = useEditor(
      {
        extensions,
        editable,
        content: shared.isValidTiptapContent(initialContent)
          ? initialContent
          : shared.EMPTY_TIPTAP_DOC,
        onCreate: ({ editor }) => {
          editor.view.dom.setAttribute("spellcheck", "false");
          editor.view.dom.setAttribute("autocomplete", "off");
          editor.view.dom.setAttribute("autocorrect", "off");
          editor.view.dom.setAttribute("autocapitalize", "off");
        },
        onUpdate: ({ editor }) => {
          onUpdateRef.current?.(editor.getJSON());
        },
        immediatelyRender: false,
        shouldRerenderOnTransaction: false,
        parseOptions: { preserveWhitespace: "full" },
      },
      [extensions],
    );

    useEffect(() => {
      if (ref && typeof ref === "object") {
        ref.current = { editor };
      }
    }, [editor, ref]);

    useEffect(() => {
      if (editor && previousContentRef.current !== initialContent) {
        previousContentRef.current = initialContent;
        if (!editor.isFocused) {
          if (shared.isValidTiptapContent(initialContent)) {
            editor.commands.setContent(initialContent, {
              parseOptions: { preserveWhitespace: "full" },
            });
          }
        }
      }
    }, [editor, initialContent]);

    useEffect(() => {
      if (editor) {
        editor.setEditable(editable);
      }
    }, [editor, editable]);

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
      <EditorContent editor={editor} className="tiptap-root" role="textbox" />
    );
  },
);

ChatEditor.displayName = "ChatEditor";

export default ChatEditor;
