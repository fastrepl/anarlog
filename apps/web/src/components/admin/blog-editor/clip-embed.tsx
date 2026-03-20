import { NodeViewWrapper, ReactNodeViewRenderer } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useCallback, useEffect, useState } from "react";

import { ClipNode as BaseClipNode } from "@hypr/tiptap/shared";

function ClipNodeView({ node, updateAttributes, selected }: NodeViewProps) {
  const [isEditing, setIsEditing] = useState(!node.attrs.src);
  const [inputValue, setInputValue] = useState(node.attrs.src || "");

  useEffect(() => {
    setInputValue(node.attrs.src || "");
  }, [node.attrs.src]);

  const handleSubmit = useCallback(() => {
    const url = inputValue.trim();
    if (!url) return;
    updateAttributes({ src: url });
    setIsEditing(false);
  }, [inputValue, updateAttributes]);

  if (isEditing || !node.attrs.src) {
    return (
      <NodeViewWrapper>
        <div className="my-4 rounded-md border border-dashed border-neutral-300 bg-neutral-50 p-4">
          <label className="mb-2 block text-xs text-neutral-500">
            YouTube Clip Embed URL
          </label>
          <div className="flex gap-2">
            <input
              type="url"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder="https://www.youtube.com/embed/..."
              className="flex-1 rounded border border-neutral-200 px-3 py-1.5 text-sm focus:border-blue-500 focus:outline-none"
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleSubmit();
                }
              }}
              autoFocus
            />
            <button
              type="button"
              onClick={handleSubmit}
              className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
            >
              Embed
            </button>
          </div>
        </div>
      </NodeViewWrapper>
    );
  }

  return (
    <NodeViewWrapper>
      <div
        className={[
          "my-4 rounded-md overflow-hidden border",
          selected ? "border-blue-500" : "border-neutral-200",
        ].join(" ")}
        onDoubleClick={() => setIsEditing(true)}
      >
        <div className="relative w-full" style={{ paddingBottom: "56.25%" }}>
          <iframe
            src={node.attrs.src}
            className="absolute inset-0 h-full w-full"
            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
            referrerPolicy="strict-origin-when-cross-origin"
            allowFullScreen
          />
        </div>
      </div>
    </NodeViewWrapper>
  );
}

export const ClipNode = BaseClipNode.extend({
  addNodeView() {
    return ReactNodeViewRenderer(ClipNodeView);
  },
});
