import { Chat } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { Avatar } from "@anlg/ui/components/avatar";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

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
    borderRadius: "3.40282e38px",
    borderStyle: "solid",
    borderWidth: "2px",
    borderColor: "#fff",
    backgroundColor: "#44403c",
    paddingInline: ".125rem",
    fontSize: "9px",
    "--tw-leading": ".75rem",
    lineHeight: ".75rem",
    color: "#fff",
  },
  style3: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: "0",
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: "0",
    position: "absolute",
    overflow: "hidden",
  },
  style4: {
    display: "flex",
    height: "3.5rem",
    flexShrink: "0",
    alignItems: "center",
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#e7e5e4",
    paddingInline: "1.25rem",
    paddingRight: "3.5rem",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: "#1c1917",
  },
  style6: {
    marginLeft: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#78716c",
  },
  style7: {
    minHeight: "0",
    flex: "1",
    overflowY: "auto",
    padding: "1rem",
  },
  style8: {
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e7e5e4",
    backgroundColor: "#fff",
    padding: ".875rem",
    "--tw-shadow": "0 1px #0000000d",
    boxShadow:
      "0 0 #0000, 0 0 #0000, 0 0 #0000, 0 0 #0000, 0 1px var(--tw-shadow-color, #0000000d)",
  },
  style9: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
  },
  style10: {
    minWidth: "0",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".75rem",
    lineHeight: "1rem",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: "#292524",
  },
  style11: {
    "--tw-font-weight": "400",
    fontWeight: "400",
    color: "#78716c",
  },
  style12: {
    marginTop: ".75rem",
    borderLeftStyle: "solid",
    borderLeftWidth: "2px",
    borderColor: "#e7e5e4",
    paddingLeft: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    "--tw-leading": "1.25rem",
    color: "#78716c",
  },
  style13: {
    marginTop: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-leading": "1.25rem",
    whiteSpace: "pre-wrap",
    color: "#292524",
  },
  style14: {
    paddingBlock: "2.5rem",
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#78716c",
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
          {...stylex.props([
            "relative grid size-9 place-items-center rounded-lg bg-transparent text-stone-600",
            "transition-colors hover:text-stone-900",
            "focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:ring-offset-2 focus-visible:outline-hidden",
          ])}
        >
          <Chat {...stylex.props(styles.style1)} aria-hidden="true" />
          {count > 0 ? (
            <span {...stylex.props(styles.style2)}>
              {count > 99 ? "99+" : count}
            </span>
          ) : null}
        </button>
      </DialogTrigger>
      <DialogContent
        showOverlay={false}
        className={[
          [
            "!top-0 !right-0 !bottom-0 !left-auto !h-dvh !w-[min(380px,100vw)] !max-w-none !translate-x-0 !translate-y-0",
            "!flex !gap-0 !rounded-none !border-y-0 !border-r-0 !border-l !border-stone-200 !bg-white !p-0",
            "flex-col overflow-hidden shadow-2xl",
          ],
        ]}
      >
        <DialogDescription {...stylex.props(styles.style3)}>
          Comments attached to this shared note.
        </DialogDescription>
        <header {...stylex.props(styles.style4)}>
          <DialogTitle {...stylex.props(styles.style5)}>Comments</DialogTitle>
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
