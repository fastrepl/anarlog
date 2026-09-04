import { Check, CircleNotch, Clock, SignIn, X } from "@phosphor-icons/react";
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { cn } from "@anlg/utils";

import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
import {
  cancelMySharedNoteAccessRequest,
  getMySharedNoteAccessRequest,
  listSharedNoteManagerAccess,
  requestSharedNoteCommentAccess,
  reviewSharedNoteAccessRequest,
} from "@/functions/shared-notes";
import { formatSharedNoteAccessRequestDescription } from "@/lib/shared-note-collaboration";
import type {
  SessionAccessRequestState,
  SessionShareAccessCursor,
  SharedNoteCapability,
} from "@/lib/shared-notes";

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
  const canCompose =
    manageAccess || capability === "commenter" || capability === "editor";
  const accessRequestQuery = useQuery({
    queryKey: accessRequestQueryKey(shareId),
    queryFn: async () => {
      const result = await getMySharedNoteAccessRequest({ data: shareId });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
      return result.request;
    },
    enabled: signedIn && !canCompose,
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
  const pendingManagerRequests = (managerAccessQuery.data?.pages ?? [])
    .flatMap((page) => page.entries)
    .filter(
      (entry) => entry.entryType === "request" && entry.status === "pending",
    );

  if (signedIn && canCompose && !manageAccess) return null;
  if (
    manageAccess &&
    !managerAccessQuery.isPending &&
    !managerAccessQuery.isError &&
    !managerAccessQuery.hasNextPage &&
    pendingManagerRequests.length === 0
  ) {
    return null;
  }

  return (
    <section
      aria-label="Comment access"
      className="surface-subtle border-color-subtle mt-8 rounded-2xl border px-4 py-4 sm:px-5"
    >
      {!signedIn ? (
        <SignInToCollaborate returnPath={returnPath} />
      ) : manageAccess ? (
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
      ) : (
        <AccessRequestPanel
          request={accessRequestQuery.data ?? null}
          error={
            accessRequestQuery.isError ||
            requestMutation.isError ||
            cancelRequestMutation.isError
          }
          loading={accessRequestQuery.isPending}
          pending={requestMutation.isPending || cancelRequestMutation.isPending}
          onCancel={(requestId) => cancelRequestMutation.mutate(requestId)}
          onRequest={() => requestMutation.mutate()}
        />
      )}
    </section>
  );
}

function SignInToCollaborate({ returnPath }: { returnPath: string }) {
  const search = new URLSearchParams({
    flow: "web",
    redirect: returnPath,
  });
  return (
    <div className="sm:flex sm:items-center sm:justify-between sm:gap-5">
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
        <SignIn className="mr-2 size-4" aria-hidden="true" />
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
      <div className="text-color-muted flex items-center gap-2 text-sm">
        <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
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
    <div className="sm:flex sm:items-center sm:justify-between sm:gap-5">
      <div>
        <p className="text-color flex items-center gap-2 font-mono text-sm font-medium">
          {isPending && <Clock className="size-4" aria-hidden="true" />}
          {isApproved ? "Comment access approved" : "Want to comment?"}
        </p>
        <p className="text-color-muted mt-1 text-sm leading-6">{description}</p>
        {error && (
          <p className="mt-2 text-sm text-red-700" role="status">
            Comment access couldn’t be updated. Try again.
          </p>
        )}
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
    return (
      <div className="text-color-muted flex items-center gap-2 text-sm">
        <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
        Checking access requests…
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-color font-mono text-sm font-medium">
        Comment access requests
      </h2>
      {requests.length > 0 && (
        <ul className="mt-3 space-y-2">
          {requests.map((request) => {
            const pending = pendingRequestId === request.entryId;
            return (
              <li
                key={request.entryId}
                className="surface rounded-xl px-3 py-3 sm:flex sm:items-center sm:justify-between sm:gap-4"
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
                    <X className="mr-1.5 size-4" aria-hidden="true" />
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
                      <CircleNotch
                        className="mr-1.5 size-4 animate-spin"
                        aria-hidden="true"
                      />
                    ) : (
                      <Check className="mr-1.5 size-4" aria-hidden="true" />
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
            <CircleNotch
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
