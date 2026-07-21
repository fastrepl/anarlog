import {
  type InfiniteData,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import {
  createSharedNoteComment,
  deleteSharedNoteComment,
  listSharedNoteComments,
} from "@/functions/shared-notes";
import type {
  SharedNoteComment,
  SharedNoteCommentAnchor,
  SharedNoteCommentCursor,
  SharedNoteCommentPage,
} from "@/lib/shared-notes";

export const sharedNoteCommentsQueryKey = (shareId: string) => [
  "shared-note-comments",
  shareId,
];

type CommentsPage = { status: "ready" | "unavailable" } & SharedNoteCommentPage;
type CommentsData = InfiniteData<CommentsPage, SharedNoteCommentCursor | null>;

export function useSharedNoteComments({
  shareId,
  enabled,
}: {
  shareId: string;
  enabled: boolean;
}) {
  return useInfiniteQuery({
    queryKey: sharedNoteCommentsQueryKey(shareId),
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
    enabled,
    initialPageParam: null as SharedNoteCommentCursor | null,
    getNextPageParam: (lastPage) =>
      lastPage.status === "ready"
        ? (lastPage.nextCursor ?? undefined)
        : undefined,
    retry: false,
  });
}

export function useCreateSharedNoteComment({
  shareId,
  snapshotRevision,
}: {
  shareId: string;
  snapshotRevision: number;
}) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      body,
      anchor,
    }: {
      body: string;
      anchor: SharedNoteCommentAnchor | null;
    }) => {
      const result = await createSharedNoteComment({
        data: { shareId, body, anchor },
      });
      if (result.status !== "ready") {
        throw new Error("comment unavailable");
      }
      return result.comment;
    },
    onMutate: async ({ body, anchor }) => {
      const queryKey = sharedNoteCommentsQueryKey(shareId);
      await queryClient.cancelQueries({ queryKey });
      const previous = queryClient.getQueryData<CommentsData>(queryKey);
      if (previous) {
        queryClient.setQueryData<CommentsData>(
          queryKey,
          insertOptimisticComment(previous, {
            commentId:
              globalThis.crypto?.randomUUID?.() ?? `optimistic-${Date.now()}`,
            isAuthor: true,
            body,
            snapshotRevision,
            anchor,
            createdAt: new Date().toISOString(),
          }),
        );
      }
      return { previous };
    },
    onError: (_error, _variables, context) => {
      if (context?.previous) {
        queryClient.setQueryData(
          sharedNoteCommentsQueryKey(shareId),
          context.previous,
        );
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: sharedNoteCommentsQueryKey(shareId),
      });
    },
  });
}

export function useDeleteSharedNoteComment({ shareId }: { shareId: string }) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (commentId: string) => {
      const result = await deleteSharedNoteComment({ data: commentId });
      if (result.status !== "ready") {
        throw new Error("comment unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: sharedNoteCommentsQueryKey(shareId),
      });
    },
  });
}

export function collectSharedNoteComments(
  data:
    | InfiniteData<
        | ({ status: "ready" } & SharedNoteCommentPage)
        | { status: "unavailable" },
        unknown
      >
    | undefined,
): SharedNoteComment[] {
  return (
    data?.pages.flatMap((page) =>
      page.status === "ready" ? page.comments : [],
    ) ?? []
  );
}

function insertOptimisticComment(
  data: CommentsData,
  comment: SharedNoteComment,
): CommentsData {
  const [firstPage, ...rest] = data.pages;
  if (!firstPage || firstPage.status !== "ready") return data;
  return {
    ...data,
    pages: [
      { ...firstPage, comments: [...firstPage.comments, comment] },
      ...rest,
    ],
  };
}
