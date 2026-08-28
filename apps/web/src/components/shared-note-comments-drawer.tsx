import { Chat } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { Avatar } from "@anlg/ui/components/avatar";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
} from "@anlg/ui/components/ui/dialog";

import { truncateSharedNoteCommentQuote } from "@/lib/shared-note-collaboration";
import { formatSharedNoteRelativeTime } from "@/lib/shared-note-presentation";
import type { SharedNoteComment } from "@/lib/shared-notes";
const styles = stylex.create({
  style1: {
    width: "19px",
    height: "19px",
  },
  style2: {
    position: "absolute",
    top: "-.125rem",
    right: "-.125rem",
    display: "grid",
    minWidth: "1rem",
    placeItems: "center",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    borderColor: colors.card,
    backgroundColor: colors.primary,
    paddingInline: ".125rem",
    fontSize: "9px",
    lineHeight: ".75rem",
    color: colors.primaryForeground,
  },
  style3: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: 0,
    position: "absolute",
    overflow: "hidden",
  },
  style4: {
    display: "flex",
    height: "3.5rem",
    flexShrink: 0,
    alignItems: "center",
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: colors.border,
    paddingInline: "1.25rem",
    paddingRight: "3.5rem",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 600,
    color: colors.foreground,
  },
  style6: {
    marginLeft: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  style7: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
    minHeight: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    overflowY: "auto",
    padding: "1rem",
  },
  style8: {
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    backgroundColor: colors.card,
    padding: ".875rem",
    boxShadow: "0 1px rgb(0 0 0 / 0.05)",
  },
  style9: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
  },
  style10: {
    minWidth: 0,
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".75rem",
    lineHeight: "1rem",
    fontWeight: 600,
    color: colors.foreground,
  },
  style11: {
    fontWeight: 400,
    color: colors.mutedForeground,
  },
  style12: {
    marginTop: ".75rem",
    borderLeftStyle: "solid",
    borderLeftWidth: "2px",
    borderColor: colors.border,
    paddingLeft: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style13: {
    marginTop: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    whiteSpace: "pre-wrap",
    color: colors.foreground,
  },
  style14: {
    paddingBlock: "2.5rem",
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  trigger: {
    backgroundColor: "transparent",
    borderRadius: radii.lg,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}, 0 0 0 4px ${colors.card}`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "grid",
    height: "2.25rem",
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    placeItems: "center",
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color",
    width: "2.25rem",
  },
  drawer: {
    backgroundColor: colors.card,
    borderBottomWidth: 0,
    borderColor: colors.border,
    borderLeftStyle: "solid",
    borderLeftWidth: "1px",
    borderRadius: 0,
    borderRightWidth: 0,
    borderTopWidth: 0,
    bottom: 0,
    boxShadow: shadows.lg,
    display: "flex",
    flexDirection: "column",
    gap: 0,
    height: "100dvh",
    left: "auto",
    maxWidth: "none",
    overflow: "hidden",
    padding: 0,
    right: 0,
    top: 0,
    transform: "none",
    width: "min(380px, 100vw)",
  },
});
export function SharedNoteCommentsDrawer({
  comments,
}: {
  comments: SharedNoteComment[];
}) {
  const count = comments.length;
  return (
    <Dialog modal={false}>
      <DialogTrigger asChild>
        <button
          type="button"
          aria-label={`Open comments${count ? ` (${count})` : ""}`}
          {...stylex.props(styles.trigger)}
        >
          <Chat {...stylex.props(styles.style1)} aria-hidden="true" />
          {count > 0 ? (
            <span {...stylex.props(styles.style2)}>
              {count > 99 ? "99+" : count}
            </span>
          ) : null}
        </button>
      </DialogTrigger>
      <DialogContent showOverlay={false} sx={styles.drawer}>
        <DialogDescription sx={styles.style3}>
          Comments attached to this shared note.
        </DialogDescription>
        <header {...stylex.props(styles.style4)}>
          <DialogTitle sx={styles.style5}>Comments</DialogTitle>
          <span {...stylex.props(styles.style6)}>{count}</span>
        </header>
        <div {...stylex.props(styles.style7)}>
          {comments.length ? (
            comments.map((comment) => (
              <article key={comment.commentId} {...stylex.props(styles.style8)}>
                <div {...stylex.props(styles.style9)}>
                  <Avatar
                    seed={
                      comment.isAuthor
                        ? "shared-note:you"
                        : "shared-note:collaborator"
                    }
                    label={comment.isAuthor ? "You" : "Collaborator"}
                    size={25}
                  />
                  <p {...stylex.props(styles.style10)}>
                    {comment.isAuthor ? "You" : "Collaborator"}{" "}
                    <time
                      {...stylex.props(styles.style11)}
                      dateTime={comment.createdAt}
                    >
                      {formatSharedNoteRelativeTime(comment.createdAt)}
                    </time>
                  </p>
                </div>
                {comment.anchor ? (
                  <p {...stylex.props(styles.style12)}>
                    {truncateSharedNoteCommentQuote(comment.anchor.quoteExact)}
                  </p>
                ) : null}
                <p {...stylex.props(styles.style13)}>{comment.body}</p>
              </article>
            ))
          ) : (
            <p {...stylex.props(styles.style14)}>No comments yet.</p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
