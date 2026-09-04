import { Avatar } from "@anlg/ui/components/avatar";
import { Chat } from "@anlg/ui/components/icons";
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
          className={cn([
            "relative grid size-9 place-items-center rounded-lg bg-transparent text-stone-600",
            "transition-colors hover:text-stone-900",
            "focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:ring-offset-2 focus-visible:outline-hidden",
          ])}
        >
          <Chat className="size-[19px]" aria-hidden="true" />
          {count > 0 ? (
            <span className="absolute -top-0.5 -right-0.5 grid min-w-4 place-items-center rounded-full border-2 border-white bg-stone-700 px-0.5 text-[9px] leading-3 text-white">
              {count > 99 ? "99+" : count}
            </span>
          ) : null}
        </button>
      </DialogTrigger>
      <DialogContent
        showOverlay={false}
        className={cn([
          "!top-0 !right-0 !bottom-0 !left-auto !h-dvh !w-[min(380px,100vw)] !max-w-none !translate-x-0 !translate-y-0",
          "!flex !gap-0 !rounded-none !border-y-0 !border-r-0 !border-l !border-stone-200 !bg-white !p-0",
          "flex-col overflow-hidden shadow-2xl",
        ])}
      >
        <DialogDescription className="sr-only">
          Comments attached to this shared note.
        </DialogDescription>
        <header className="flex h-14 shrink-0 items-center border-b border-stone-200 px-5 pr-14">
          <DialogTitle className="text-sm font-semibold text-stone-900">
            Comments
          </DialogTitle>
          <span className="ml-2 text-xs text-stone-500">{count}</span>
        </header>
        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-4">
          {comments.length ? (
            comments.map((comment) => (
              <article
                key={comment.commentId}
                className="rounded-2xl border border-stone-200 bg-white p-3.5 shadow-xs"
              >
                <div className="flex items-center gap-2">
                  <Avatar
                    seed={
                      comment.isAuthor
                        ? "shared-note:you"
                        : "shared-note:collaborator"
                    }
                    label={comment.isAuthor ? "You" : "Collaborator"}
                    size={25}
                  />
                  <p className="min-w-0 truncate text-xs font-semibold text-stone-800">
                    {comment.isAuthor ? "You" : "Collaborator"}{" "}
                    <time
                      className="font-normal text-stone-500"
                      dateTime={comment.createdAt}
                    >
                      {formatSharedNoteRelativeTime(comment.createdAt)}
                    </time>
                  </p>
                </div>
                {comment.anchor ? (
                  <p className="mt-3 border-l-2 border-stone-200 pl-2 text-xs leading-5 text-stone-500">
                    {truncateSharedNoteCommentQuote(comment.anchor.quoteExact)}
                  </p>
                ) : null}
                <p className="mt-2 text-sm leading-5 whitespace-pre-wrap text-stone-800">
                  {comment.body}
                </p>
              </article>
            ))
          ) : (
            <p className="py-10 text-center text-sm text-stone-500">
              No comments yet.
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
