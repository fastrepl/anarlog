import Image from "@tiptap/extension-image";
import { NodeViewWrapper, ReactNodeViewRenderer } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useCallback, useEffect, useRef, useState } from "react";

function ImageNodeView({ node, updateAttributes, selected }: NodeViewProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const [altText, setAltText] = useState(node.attrs.alt || "");
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setAltText(node.attrs.alt || "");
  }, [node.attrs.alt]);

  const handleAltChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newAlt = e.target.value;
      setAltText(newAlt);
      updateAttributes({ alt: newAlt });
    },
    [updateAttributes],
  );

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      inputRef.current?.blur();
    }
  }, []);

  const showAltField = isHovered || isFocused;

  return (
    <NodeViewWrapper className="relative">
      <div
        ref={containerRef}
        className="relative inline-block w-full"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <img
          src={node.attrs.src}
          alt={node.attrs.alt || ""}
          title={node.attrs.title || undefined}
          className={[
            "tiptap-image max-w-full",
            selected ? "ring-2 ring-blue-500" : "",
          ].join(" ")}
          draggable={false}
        />
        {showAltField && (
          <div className="absolute right-2 bottom-2 left-2 rounded-md border border-neutral-200 bg-white/95 p-2 shadow-lg backdrop-blur-sm">
            <label className="flex items-center gap-2">
              <span className="text-xs whitespace-nowrap text-neutral-500">
                Alt text:
              </span>
              <input
                ref={inputRef}
                type="text"
                value={altText}
                onChange={handleAltChange}
                onKeyDown={handleKeyDown}
                onFocus={() => setIsFocused(true)}
                onBlur={() => setIsFocused(false)}
                placeholder="Describe this image..."
                className="flex-1 border-none bg-transparent text-sm text-neutral-700 outline-none placeholder:text-neutral-400"
              />
            </label>
          </div>
        )}
      </div>
    </NodeViewWrapper>
  );
}

export const BlogImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      attachmentId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-attachment-id"),
        renderHTML: (attributes) => {
          if (!attributes.attachmentId) {
            return {};
          }
          return { "data-attachment-id": attributes.attachmentId };
        },
      },
    };
  },

  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView);
  },

  parseMarkdown: (token: { href?: string; text?: string; title?: string }) => {
    return {
      type: "image",
      attrs: {
        src: token.href || "",
        alt: token.text || "",
        title: token.title || null,
      },
    };
  },

  renderMarkdown: (node: {
    attrs?: { src?: string; alt?: string; title?: string };
  }) => {
    const src = node.attrs?.src || "";
    const alt = node.attrs?.alt || "";
    const title = node.attrs?.title;
    return title ? `![${alt}](${src} "${title}")` : `![${alt}](${src})`;
  },
});
