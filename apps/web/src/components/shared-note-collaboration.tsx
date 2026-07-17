import { useForm } from "@tanstack/react-form";
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  CheckIcon,
  Clock3Icon,
  LoaderCircleIcon,
  LogInIcon,
  MessageSquareIcon,
  SendIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";

import { cn } from "@hypr/utils";

import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
import {
  cancelMySharedNoteAccessRequest,
  createSharedNoteComment,
  deleteSharedNoteComment,
  getMySharedNoteAccessRequest,
  listSharedNoteComments,
  listSharedNoteManagerAccess,
  requestSharedNoteCommentAccess,
  reviewSharedNoteAccessRequest,
} from "@/functions/shared-notes";
import {
  canComposeSharedNoteComments,
  formatSharedNoteAccessRequestDescription,
  hasSharedNoteCollaborationAccess,
  MAX_SHARED_NOTE_COMMENT_BYTES,
  validateSharedNoteCommentBody,
} from "@/lib/shared-note-collaboration";
import type {
  SessionAccessRequestState,
  SessionShareAccessCursor,
  SharedNoteCommentCursor,
  SharedNoteCapability,
} from "@/lib/shared-notes";

const commentsQueryKey = (shareId: string) => ["shared-note-comments", shareId];
const accessRequestQueryKey = (shareId: string) => [
  "shared-note-access-request",
  shareId,
];
const managerAccessQueryKey = (shareId: string) => [
  "shared-note-manager-access",
  shareId,
];

export function SharedNoteCollaboration({
  capability,
  currentUserId,
  manageAccess,
  returnPath,
  shareId,
}: {
  capability: SharedNoteCapability;
  currentUserId: string | null;
  manageAccess: boolean;
  returnPath: string;
  shareId: string;
}) {
  const queryClient = useQueryClient();
  const signedIn = currentUserId !== null;
  const commentsQuery = useInfiniteQuery({
    queryKey: commentsQueryKey(shareId),
    queryFn: async ({ pageParam }) => {
      const result = await listSharedNoteComments({
        data: {
          shareId,
          beforeCreatedAt: pageParam?.beforeCreatedAt ?? null,
          beforeCommentId: pageParam?.beforeCommentId ?? null,
        },
      });
      if (result.status === "error") {
        throw new Error("comments unavailable");
      }
      return result;
    },
    enabled: signedIn,
    initialPageParam: null as SharedNoteCommentCursor | null,
    getNextPageParam: (lastPage) =>
      lastPage.status === "ready"
        ? (lastPage.nextCursor ?? undefined)
        : undefined,
    retry: false,
  });
  const accessRequestQuery = useQuery({
    queryKey: accessRequestQueryKey(shareId),
    queryFn: async () => {
      const result = await getMySharedNoteAccessRequest({ data: shareId });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
      return result.request;
    },
    enabled: signedIn && !manageAccess,
    retry: false,
  });
  const managerAccessQuery = useInfiniteQuery({
    queryKey: managerAccessQueryKey(shareId),
    queryFn: async ({ pageParam }) => {
      const result = await listSharedNoteManagerAccess({
        data: {
          shareId,
          beforeCreatedAt: pageParam?.beforeCreatedAt ?? null,
          beforeEntryId: pageParam?.beforeEntryId ?? null,
        },
      });
      if (result.status !== "ready") {
        throw new Error("manager access unavailable");
      }
      return result;
    },
    enabled: signedIn && manageAccess,
    initialPageParam: null as SessionShareAccessCursor | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    retry: false,
  });
  const createMutation = useMutation({
    mutationFn: async (body: string) => {
      const result = await createSharedNoteComment({
        data: { shareId, body },
      });
      if (result.status !== "ready") {
        throw new Error("comment unavailable");
      }
      return result.comment;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: commentsQueryKey(shareId),
      });
    },
  });
  const deleteMutation = useMutation({
    mutationFn: async (commentId: string) => {
      const result = await deleteSharedNoteComment({ data: commentId });
      if (result.status !== "ready") {
        throw new Error("comment unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: commentsQueryKey(shareId),
      });
    },
  });
  const requestMutation = useMutation({
    mutationFn: async () => {
      const result = await requestSharedNoteCommentAccess({ data: shareId });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
      return result.request;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: accessRequestQueryKey(shareId),
      });
    },
  });
  const cancelRequestMutation = useMutation({
    mutationFn: async (requestId: string) => {
      const result = await cancelMySharedNoteAccessRequest({ data: requestId });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: accessRequestQueryKey(shareId),
      });
    },
  });
  const reviewMutation = useMutation({
    mutationFn: async ({
      decision,
      capability,
      requestId,
    }: {
      decision: "approved" | "denied";
      capability: SharedNoteCapability;
      requestId: string;
    }) => {
      const result = await reviewSharedNoteAccessRequest({
        data:
          decision === "approved"
            ? { capability, decision, requestId }
            : { decision, requestId },
      });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: managerAccessQueryKey(shareId),
      });
    },
  });
  const commentForm = useForm({
    defaultValues: { body: "" },
    onSubmit: async ({ formApi, value }) => {
      const comment = validateSharedNoteCommentBody(value.body);
      if (!comment.valid) return;
      await createMutation.mutateAsync(comment.body);
      formApi.reset();
    },
  });

  const hasCollaborationAccess = hasSharedNoteCollaborationAccess(
    commentsQuery.data?.pages[0],
  );
  const canCompose = canComposeSharedNoteComments({
    capability,
    hasCollaborationAccess,
    manageAccess,
  });
  const comments = [...(commentsQuery.data?.pages ?? [])]
    .reverse()
    .flatMap((page) => (page.status === "ready" ? page.comments : []));
  const accessRequest = accessRequestQuery.data ?? null;
  const pendingManagerRequests = (managerAccessQuery.data?.pages ?? [])
    .flatMap((page) => page.entries)
    .filter(
      (entry) => entry.entryType === "request" && entry.status === "pending",
    );

  return (
    <section
      aria-labelledby="shared-note-comments-heading"
      className="surface border-color-subtle mt-6 rounded-3xl border px-6 py-7 shadow-sm sm:px-10"
    >
      <div className="flex items-start justify-between gap-5">
        <div>
          <div className="text-color flex items-center gap-2">
            <MessageSquareIcon className="size-5" aria-hidden="true" />
            <h2
              id="shared-note-comments-heading"
              className="font-mono text-lg font-medium"
            >
              Comments
            </h2>
          </div>
          <p className="text-color-muted mt-1 text-sm leading-6">
            A private conversation for people with access to this note.
          </p>
        </div>
        {hasCollaborationAccess && commentsQuery.isSuccess && (
          <span className="surface-subtle text-color-muted rounded-full px-3 py-1 font-mono text-xs">
            {comments.length}
            {commentsQuery.hasNextPage ? "+" : ""}
          </span>
        )}
      </div>

      {!signedIn ? (
        <SignInToCollaborate returnPath={returnPath} />
      ) : (
        <>
          {commentsQuery.isPending ||
          commentsQuery.isError ||
          hasCollaborationAccess ? (
            <CommentList
              comments={comments}
              error={
                (commentsQuery.isError &&
                  !commentsQuery.isFetchNextPageError) ||
                deleteMutation.isError
              }
              hasEarlier={commentsQuery.hasNextPage}
              loading={commentsQuery.isPending}
              loadingEarlier={commentsQuery.isFetchingNextPage}
              loadEarlierError={commentsQuery.isFetchNextPageError}
              manageAccess={manageAccess}
              deletingCommentId={deleteMutation.variables ?? null}
              deletePending={deleteMutation.isPending}
              onDelete={(commentId) => deleteMutation.mutate(commentId)}
              onLoadEarlier={() => void commentsQuery.fetchNextPage()}
            />
          ) : (
            <p className="surface-subtle text-color-muted mt-6 rounded-2xl px-4 py-4 text-sm leading-6">
              Comments are available after the note owner grants your account
              access.
            </p>
          )}

          {canCompose && (
            <form
              className="border-color-subtle mt-6 border-t pt-5"
              onSubmit={(event) => {
                event.preventDefault();
                event.stopPropagation();
                void commentForm.handleSubmit();
              }}
            >
              <commentForm.Field name="body">
                {(field) => {
                  const comment = validateSharedNoteCommentBody(
                    field.state.value,
                  );
                  const tooLong =
                    comment.byteLength > MAX_SHARED_NOTE_COMMENT_BYTES;
                  const errorId = "shared-note-comment-body-error";
                  return (
                    <>
                      <label
                        htmlFor="shared-note-comment-body"
                        className="text-color font-mono text-sm font-medium"
                      >
                        Add a comment
                      </label>
                      <textarea
                        id="shared-note-comment-body"
                        className={cn([
                          "surface-subtle border-color-subtle text-color mt-2 min-h-24 w-full resize-y rounded-2xl border px-4 py-3",
                          "placeholder:text-color-muted text-sm leading-6 focus:border-stone-400 focus:ring-2 focus:ring-stone-300 focus:outline-hidden",
                        ])}
                        aria-describedby={tooLong ? errorId : undefined}
                        aria-invalid={tooLong}
                        placeholder="Share a thought about this note…"
                        required
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(event) =>
                          field.handleChange(event.target.value)
                        }
                      />
                      {tooLong && (
                        <p
                          id={errorId}
                          className="mt-2 text-sm text-red-700"
                          role="alert"
                        >
                          Comment is too long (
                          {comment.byteLength.toLocaleString()}/
                          {MAX_SHARED_NOTE_COMMENT_BYTES.toLocaleString()}{" "}
                          bytes).
                        </p>
                      )}
                      <div className="mt-3 flex items-center justify-between gap-4">
                        <p className="text-color-muted text-xs">
                          Visible only to people who can open this note.
                        </p>
                        <button
                          type="submit"
                          className={sharedPrimaryButtonClassName}
                          disabled={createMutation.isPending || !comment.valid}
                        >
                          {createMutation.isPending ? (
                            <LoaderCircleIcon
                              className="mr-2 size-4 animate-spin"
                              aria-hidden="true"
                            />
                          ) : (
                            <SendIcon
                              className="mr-2 size-4"
                              aria-hidden="true"
                            />
                          )}
                          Comment
                        </button>
                      </div>
                    </>
                  );
                }}
              </commentForm.Field>
              {createMutation.isError && (
                <p className="mt-3 text-sm text-red-700" role="status">
                  Your comment couldn’t be added. Try again.
                </p>
              )}
            </form>
          )}

          {!canCompose &&
            !manageAccess &&
            !commentsQuery.isPending &&
            !commentsQuery.isError && (
              <AccessRequestPanel
                request={accessRequest}
                error={
                  accessRequestQuery.isError ||
                  requestMutation.isError ||
                  cancelRequestMutation.isError
                }
                loading={accessRequestQuery.isPending}
                pending={
                  requestMutation.isPending || cancelRequestMutation.isPending
                }
                onCancel={(requestId) =>
                  cancelRequestMutation.mutate(requestId)
                }
                onRequest={() => requestMutation.mutate()}
              />
            )}

          {manageAccess && (
            <ManagerRequests
              error={
                managerAccessQuery.isError ||
                managerAccessQuery.isFetchNextPageError ||
                reviewMutation.isError
              }
              hasEarlierRequests={managerAccessQuery.hasNextPage}
              loading={managerAccessQuery.isPending}
              loadingEarlier={managerAccessQuery.isFetchingNextPage}
              pendingRequestId={
                reviewMutation.isPending
                  ? (reviewMutation.variables?.requestId ?? null)
                  : null
              }
              requests={pendingManagerRequests}
              onLoadEarlier={() => managerAccessQuery.fetchNextPage()}
              onReview={(requestId, decision, capability) =>
                reviewMutation.mutate({ capability, decision, requestId })
              }
            />
          )}
        </>
      )}
    </section>
  );
}

function CommentList({
  comments,
  deletePending,
  deletingCommentId,
  error,
  hasEarlier,
  loading,
  loadingEarlier,
  loadEarlierError,
  manageAccess,
  onDelete,
  onLoadEarlier,
}: {
  comments: Array<{
    body: string;
    commentId: string;
    createdAt: string;
    isAuthor: boolean;
  }>;
  deletePending: boolean;
  deletingCommentId: string | null;
  error: boolean;
  hasEarlier: boolean;
  loading: boolean;
  loadingEarlier: boolean;
  loadEarlierError: boolean;
  manageAccess: boolean;
  onDelete: (commentId: string) => void;
  onLoadEarlier: () => void;
}) {
  if (loading) {
    return (
      <div className="text-color-muted mt-6 flex items-center gap-2 text-sm">
        <LoaderCircleIcon className="size-4 animate-spin" aria-hidden="true" />
        Loading comments…
      </div>
    );
  }
  if (error) {
    return (
      <p className="mt-6 text-sm text-red-700" role="status">
        Comments couldn’t be loaded right now.
      </p>
    );
  }
  if (!comments.length) {
    return (
      <p className="surface-subtle text-color-muted mt-6 rounded-2xl px-4 py-4 text-sm leading-6">
        No comments yet.
      </p>
    );
  }

  return (
    <>
      {hasEarlier && (
        <button
          type="button"
          className={cn([sharedSecondaryButtonClassName, "mt-6"])}
          disabled={loadingEarlier}
          onClick={onLoadEarlier}
        >
          {loadingEarlier && (
            <LoaderCircleIcon
              className="size-4 animate-spin"
              aria-hidden="true"
            />
          )}
          Load earlier comments
        </button>
      )}
      {loadEarlierError && (
        <p className="mt-3 text-sm text-red-700" role="status">
          Earlier comments couldn’t be loaded. Please try again.
        </p>
      )}
      <ol className="border-color-subtle mt-6 divide-y border-y">
        {comments.map((comment) => {
          const canDelete = comment.isAuthor || manageAccess;
          const deleting =
            deletePending && deletingCommentId === comment.commentId;
          return (
            <li key={comment.commentId} className="py-5">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <p className="text-color font-mono text-sm font-medium">
                    {comment.isAuthor ? "You" : "Collaborator"}
                  </p>
                  <time
                    className="text-color-muted mt-0.5 block text-xs"
                    dateTime={comment.createdAt}
                  >
                    {formatCommentDate(comment.createdAt)}
                  </time>
                </div>
                {canDelete && (
                  <button
                    type="button"
                    className="text-color-muted hover:text-color rounded-full p-2 transition-colors focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden disabled:opacity-50"
                    aria-label="Delete comment"
                    disabled={deletePending}
                    onClick={() => onDelete(comment.commentId)}
                  >
                    {deleting ? (
                      <LoaderCircleIcon
                        className="size-4 animate-spin"
                        aria-hidden="true"
                      />
                    ) : (
                      <Trash2Icon className="size-4" aria-hidden="true" />
                    )}
                  </button>
                )}
              </div>
              <p className="text-color mt-3 text-sm leading-6 whitespace-pre-wrap">
                {comment.body}
              </p>
            </li>
          );
        })}
      </ol>
    </>
  );
}

function SignInToCollaborate({ returnPath }: { returnPath: string }) {
  const search = new URLSearchParams({
    flow: "web",
    redirect: returnPath,
  });
  return (
    <div className="surface-subtle border-color-subtle mt-6 rounded-2xl border px-4 py-5 sm:flex sm:items-center sm:justify-between sm:gap-5">
      <div>
        <p className="text-color font-mono text-sm font-medium">
          Sign in to join the conversation
        </p>
        <p className="text-color-muted mt-1 text-sm leading-6">
          Sign in to view comments or request permission to comment.
        </p>
      </div>
      <a
        href={`/auth/?${search.toString()}`}
        className={cn([sharedPrimaryButtonClassName, "mt-4 sm:mt-0"])}
      >
        <LogInIcon className="mr-2 size-4" aria-hidden="true" />
        Sign in
      </a>
    </div>
  );
}

function AccessRequestPanel({
  error,
  loading,
  onCancel,
  onRequest,
  pending,
  request,
}: {
  error: boolean;
  loading: boolean;
  onCancel: (requestId: string) => void;
  onRequest: () => void;
  pending: boolean;
  request: SessionAccessRequestState | null;
}) {
  if (loading) {
    return (
      <div className="border-color-subtle text-color-muted mt-6 flex items-center gap-2 border-t pt-5 text-sm">
        <LoaderCircleIcon className="size-4 animate-spin" aria-hidden="true" />
        Checking comment access…
      </div>
    );
  }

  const isPending = request?.status === "pending";
  const isApproved = request?.status === "approved";
  const description = isPending
    ? "The note owner can approve or decline your request."
    : isApproved
      ? "Your request was approved. Reload this note to use your new access."
      : request?.status === "denied"
        ? "Your previous request was declined. You can send a new request if needed."
        : request?.status === "cancelled"
          ? "Your previous request was cancelled."
          : "Ask the note owner for permission to join the conversation.";

  return (
    <div className="border-color-subtle mt-6 border-t pt-5">
      <div className="sm:flex sm:items-center sm:justify-between sm:gap-5">
        <div>
          <p className="text-color flex items-center gap-2 font-mono text-sm font-medium">
            {isPending && <Clock3Icon className="size-4" aria-hidden="true" />}
            {isApproved ? "Comment access approved" : "Want to comment?"}
          </p>
          <p className="text-color-muted mt-1 text-sm leading-6">
            {description}
          </p>
        </div>
        <div className="mt-4 flex shrink-0 gap-2 sm:mt-0">
          {isPending ? (
            <button
              type="button"
              className={sharedSecondaryButtonClassName}
              disabled={pending}
              onClick={() => onCancel(request.requestId)}
            >
              Cancel request
            </button>
          ) : isApproved ? (
            <button
              type="button"
              className={sharedPrimaryButtonClassName}
              onClick={() => window.location.reload()}
            >
              Reload note
            </button>
          ) : (
            <button
              type="button"
              className={sharedPrimaryButtonClassName}
              disabled={pending}
              onClick={onRequest}
            >
              {pending ? "Requesting…" : "Request comment access"}
            </button>
          )}
        </div>
      </div>
      {error && (
        <p className="mt-3 text-sm text-red-700" role="status">
          Comment access couldn’t be updated. Try again.
        </p>
      )}
    </div>
  );
}

function ManagerRequests({
  error,
  hasEarlierRequests,
  loading,
  loadingEarlier,
  onLoadEarlier,
  onReview,
  pendingRequestId,
  requests,
}: {
  error: boolean;
  hasEarlierRequests: boolean;
  loading: boolean;
  loadingEarlier: boolean;
  onLoadEarlier: () => void;
  onReview: (
    requestId: string,
    decision: "approved" | "denied",
    capability: SharedNoteCapability,
  ) => void;
  pendingRequestId: string | null;
  requests: Array<{
    capability: SharedNoteCapability;
    entryId: string;
    userEmail: string;
  }>;
}) {
  if (loading) {
    return null;
  }
  if (!requests.length && !hasEarlierRequests && !error) {
    return null;
  }

  return (
    <div className="border-color-subtle mt-6 border-t pt-5">
      <h3 className="text-color font-mono text-sm font-medium">
        Access requests
      </h3>
      {requests.length > 0 && (
        <ul className="mt-3 space-y-2">
          {requests.map((request) => {
            const pending = pendingRequestId === request.entryId;
            return (
              <li
                key={request.entryId}
                className="surface-subtle rounded-2xl px-4 py-3 sm:flex sm:items-center sm:justify-between sm:gap-4"
              >
                <div>
                  <p className="text-color text-sm font-medium">
                    {request.userEmail}
                  </p>
                  <p className="text-color-muted mt-0.5 text-xs">
                    {formatSharedNoteAccessRequestDescription(
                      request.capability,
                    )}
                  </p>
                </div>
                <div className="mt-3 flex gap-2 sm:mt-0">
                  <button
                    type="button"
                    className={cn([
                      sharedSecondaryButtonClassName,
                      "min-h-9 px-3",
                    ])}
                    disabled={pendingRequestId !== null}
                    onClick={() =>
                      onReview(request.entryId, "denied", request.capability)
                    }
                  >
                    <XIcon className="mr-1.5 size-4" aria-hidden="true" />
                    Deny
                  </button>
                  <button
                    type="button"
                    className={cn([
                      sharedPrimaryButtonClassName,
                      "min-h-9 px-3",
                    ])}
                    disabled={pendingRequestId !== null}
                    onClick={() =>
                      onReview(request.entryId, "approved", request.capability)
                    }
                  >
                    {pending ? (
                      <LoaderCircleIcon
                        className="mr-1.5 size-4 animate-spin"
                        aria-hidden="true"
                      />
                    ) : (
                      <CheckIcon className="mr-1.5 size-4" aria-hidden="true" />
                    )}
                    Approve
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {hasEarlierRequests && (
        <button
          type="button"
          className={cn([sharedSecondaryButtonClassName, "mt-3"])}
          disabled={loadingEarlier}
          onClick={onLoadEarlier}
        >
          {loadingEarlier && (
            <LoaderCircleIcon
              className="mr-2 size-4 animate-spin"
              aria-hidden="true"
            />
          )}
          {loadingEarlier ? "Loading…" : "Load earlier requests"}
        </button>
      )}
      {error && (
        <p className="mt-3 text-sm text-red-700" role="status">
          Access requests couldn’t be updated. Try again.
        </p>
      )}
    </div>
  );
}

function formatCommentDate(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
