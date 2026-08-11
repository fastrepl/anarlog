import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import {
  type Permission,
  type PermissionGuidance,
  commands as permissionsCommands,
  type PermissionStatus,
  type Result,
} from "@anlg/plugin-permissions";

function unwrap<T>(result: Result<T, string>): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

/**
 * Static "how to grant" guidance for a permission type. Returns `null` for
 * permissions with a normal system prompt — only list-based Privacy panes,
 * where the user has to enable the app themselves, get an assisted overlay.
 *
 * The native table never changes at runtime, so this is cached forever.
 */
export function usePermissionGuidance(type: Permission) {
  const query = useQuery({
    queryKey: ["permissionGuidance", type],
    queryFn: async () =>
      unwrap(await permissionsCommands.permissionGuidance(type)),
    staleTime: Infinity,
    gcTime: Infinity,
  });

  return query.data?.assisted ? (query.data as PermissionGuidance) : null;
}

/** Dismiss the assisted drag overlay, if one is showing. */
export async function closePermissionAssistant() {
  unwrap(await permissionsCommands.closePermissionAssistant());
}

// macOS keeps reporting the old value for a moment after a grant, so an optimistic
// status bridges that gap. It must expire, otherwise one lucky probe would pin the
// row to authorized and disable the retry button forever.
const OPTIMISTIC_GRACE_MS = 1000;

export function usePermission(type: Permission) {
  const [optimistic, setOptimistic] = useState<{
    status: PermissionStatus;
    since: number;
  } | null>(null);
  const statusQuery = useQuery({
    queryKey: [`${type}Permission`],
    queryFn: () => permissionsCommands.checkPermission(type),
    refetchInterval: 1000,
  });
  const status: PermissionStatus | undefined =
    statusQuery.data?.status === "ok"
      ? statusQuery.data.data
      : statusQuery.data
        ? "denied"
        : undefined;

  const optimisticStatus =
    optimistic &&
    statusQuery.dataUpdatedAt <= optimistic.since + OPTIMISTIC_GRACE_MS
      ? optimistic.status
      : null;
  const effectiveStatus = optimisticStatus ?? status;

  const requestMutation = useMutation({
    mutationFn: async () =>
      unwrap(await permissionsCommands.requestPermission(type)),
    onSuccess: async () => {
      if (type === "systemAudio" || type === "screenRecording") {
        setOptimistic({ status: "authorized", since: Date.now() });
        setTimeout(() => void statusQuery.refetch(), 1000);
        return;
      }
      setOptimistic(null);
      setTimeout(() => statusQuery.refetch(), 1000);
    },
    onError: () => {
      setOptimistic(null);
    },
  });

  const resetMutation = useMutation({
    mutationFn: async () =>
      unwrap(await permissionsCommands.resetPermission(type)),
    onSuccess: () => {
      setOptimistic(null);
      setTimeout(() => statusQuery.refetch(), 1000);
    },
  });

  const isPending = requestMutation.isPending || resetMutation.isPending;

  const open = async () => {
    unwrap(await permissionsCommands.openPermission(type));
  };

  const request = () => {
    requestMutation.mutate();
  };

  const reset = () => {
    resetMutation.mutate();
  };

  return {
    status: effectiveStatus,
    confirmedStatus: status,
    isPending,
    // A recovered capability must not keep rendering the diagnostics of an older
    // failed request.
    error:
      effectiveStatus === "authorized"
        ? null
        : (requestMutation.error?.message ??
          (statusQuery.data?.status === "error"
            ? statusQuery.data.error
            : statusQuery.error instanceof Error
              ? statusQuery.error.message
              : null)),
    open,
    request,
    reset,
  };
}
