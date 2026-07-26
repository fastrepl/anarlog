import { parseImageMetadata } from "@hypr/editor/node-views";
import { cn } from "@hypr/utils";

// Typography comes from the shared `.note-typography` scope (see
// packages/editor styles) so the streaming view matches the editor exactly;
// only structural concerns live here.
export const streamdownComponents = {
  img: (props: React.ImgHTMLAttributes<HTMLImageElement>) => {
    const { editorWidth, title } = parseImageMetadata(props.title);

    return (
      <img
        {...props}
        title={title}
        className={cn(["max-w-full", props.className])}
        style={{
          ...(editorWidth ? { width: `${editorWidth}%` } : {}),
          ...(props.style || {}),
        }}
      />
    );
  },
} as const;
