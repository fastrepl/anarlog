import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

export type FileUploadResult = {
  url: string;
  attachmentId: string;
  path: string;
};

export type FileHandlerConfig = {
  onDrop?: (files: File[], pos?: number) => boolean | void;
  onPaste?: (files: File[]) => boolean | void;
  onFileUpload?: (file: File) => Promise<FileUploadResult>;
};

const IMAGE_MIME_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

function isImageFile(file: File) {
  return IMAGE_MIME_TYPES.includes(file.type);
}

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

  function insertFileAttachment(
    view: EditorView,
    attrs: {
      attachmentId: string;
      name: string;
      mimeType: string;
      src: string;
      path: string;
      size: number;
    },
    pos?: number,
  ) {
    const attachmentType = view.state.schema.nodes.fileAttachment;
    if (!attachmentType) return;
    const node = attachmentType.create(attrs);
    const tr =
      pos != null
        ? view.state.tr.insert(pos, node)
        : view.state.tr.replaceSelectionWith(node);
    view.dispatch(tr);
  }

  async function handleFiles(view: EditorView, files: File[], pos?: number) {
    for (const file of files) {
      if (config.onFileUpload) {
        try {
          const result = await config.onFileUpload(file);
          if (isImageFile(file)) {
            insertImage(view, result.url, result.attachmentId, pos);
          } else {
            insertFileAttachment(
              view,
              {
                attachmentId: result.attachmentId,
                name: file.name,
                mimeType: file.type,
                src: result.url,
                path: result.path,
                size: file.size,
              },
              pos,
            );
          }
        } catch (error) {
          console.error("Failed to upload file:", error);
        }
      } else if (isImageFile(file)) {
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
        const files = Array.from(event.dataTransfer?.files ?? []);
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
        const files = Array.from(event.clipboardData?.files ?? []);
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
