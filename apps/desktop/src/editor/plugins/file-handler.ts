import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

export type FileHandlerConfig = {
  onDrop?: (files: File[], pos?: number) => boolean | void;
  onPaste?: (files: File[]) => boolean | void;
  onImageUpload?: (
    file: File,
  ) => Promise<{ url: string; attachmentId: string }>;
};

const IMAGE_MIME_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

export function fileHandlerPlugin(config: FileHandlerConfig) {
  function insertImage(
    view: EditorView,
    url: string,
    attachmentId: string | null,
    pos?: number,
  ) {
    const imageType = view.state.schema.nodes.image;
    const node = imageType.create({ src: url, attachmentId });
    const tr =
      pos != null
        ? view.state.tr.insert(pos, node)
        : view.state.tr.replaceSelectionWith(node);
    view.dispatch(tr);
  }

  async function handleFiles(view: EditorView, files: File[], pos?: number) {
    for (const file of files) {
      if (!IMAGE_MIME_TYPES.includes(file.type)) continue;

      if (config.onImageUpload) {
        try {
          const { url, attachmentId } = await config.onImageUpload(file);
          insertImage(view, url, attachmentId, pos);
        } catch (error) {
          console.error("Failed to upload image:", error);
        }
      } else {
        const reader = new FileReader();
        reader.readAsDataURL(file);
        reader.onload = () => {
          insertImage(view, reader.result as string, null, pos);
        };
      }
    }
  }

  return new Plugin({
    key: new PluginKey("fileHandler"),
    props: {
      handleDrop(view, event) {
        const files = Array.from(event.dataTransfer?.files ?? []).filter((f) =>
          IMAGE_MIME_TYPES.includes(f.type),
        );
        if (files.length === 0) return false;

        event.preventDefault();
        const pos = view.posAtCoords({
          left: event.clientX,
          top: event.clientY,
        })?.pos;

        if (config.onDrop) {
          const result = config.onDrop(files, pos);
          if (result === false) return false;
        }

        handleFiles(view, files, pos);
        return true;
      },

      handlePaste(view, event) {
        const files = Array.from(event.clipboardData?.files ?? []).filter((f) =>
          IMAGE_MIME_TYPES.includes(f.type),
        );
        if (files.length === 0) return false;

        if (config.onPaste) {
          const result = config.onPaste(files);
          if (result === false) return false;
        }

        handleFiles(view, files);
        return true;
      },
    },
  });
}
