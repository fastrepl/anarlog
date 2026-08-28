import * as stylex from "@stylexjs/stylex";

import { parseImageMetadata } from "@anlg/editor/node-views";
import { markdownComponents } from "@anlg/ui/components/markdown";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

// Typography comes from the shared `.note-typography` scope (see
// packages/editor styles) so the streaming view matches the editor exactly;
// only structural concerns live here.
export const streamdownComponents = {
  ...markdownComponents,
  // Streamdown's built-in li carries `py-1` (4px/side); the editor's rhythm is
  // 0.125em/side on `li > p`. Padding the li itself (with `[&>p]:inline` so
  // loose lists don't double-pad) keeps the flat streaming gap identical.
  // A nested list breaks that equivalence: the li's padding wraps the whole
  // li (text + sublist), while the editor pads only the text row. Shifting
  // the li's share onto the sublist via margins (mt adds the missing half
  // above, -mb cancels the li's bottom pad below) restores the 0.25em rhythm
  // without stretching the sublist's guide rail. `!` because note-typography's
  // `ul { margin-block: 0 }` reset is unlayered and outranks plain utilities.
  li: ({
    className,
    style,
    ...props
  }: React.LiHTMLAttributes<HTMLLIElement>) => (
    <li {...props} {...mergeStyleXProps(styles.listItem, className, style)} />
  ),
  img: (props: React.ImgHTMLAttributes<HTMLImageElement>) => {
    const { editorWidth, title } = parseImageMetadata(props.title);

    return (
      <img
        {...props}
        title={title}
        {...mergeStyleXProps(styles.image, props.className, {
          ...(editorWidth ? { width: `${editorWidth}%` } : {}),
          ...(props.style || {}),
        })}
      />
    );
  },
} as const;

const styles = stylex.create({
  image: {
    maxWidth: "100%",
  },
  listItem: {
    display: {
      default: null,
      ":is(*) > p": "inline",
    },
    marginBottom: {
      default: null,
      ":is(*) > :is(ul, ol)": "-0.125em !important",
    },
    marginTop: {
      default: null,
      ":is(*) > :is(ul, ol)": "0.125em !important",
    },
    paddingBlock: "0.125em",
  },
});
