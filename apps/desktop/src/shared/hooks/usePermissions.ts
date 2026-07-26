import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import {
  type Permission,
  commands as permissionsCommands,
  type PermissionStatus,
  type Result,
} from "@hypr/plugin-permissions";

function unwrap<T>(result: Result<T, string>): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

export function usePermission(type: Permission) {
  const [optimisticStatus, setOptimisticStatus] =
    useState<PermissionStatus | null>(null);
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

  const requestMutation = useMutation({
    mutationFn: async () =>
      unwrap(await permissionsCommands.requestPermission(type)),
    onSuccess: async () => {
      if (type === "systemAudio" || type === "screenRecording") {
        setOptimisticStatus("authorized");
        setTimeout(() => void statusQuery.refetch(), 1000);
        return;
      }
      setOptimisticStatus(null);
      setTimeout(() => statusQuery.refetch(), 1000);
    },
    onError: () => {
      setOptimisticStatus(null);
    },
  });

  const resetMutation = useMutation({
    mutationFn: async () =>
      unwrap(await permissionsCommands.resetPermission(type)),
    onSuccess: () => {
      setOptimisticStatus(null);
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
    status: optimisticStatus ?? status,
    isPending,
    error:
      requestMutation.error?.message ??
      (statusQuery.data?.status === "error"
        ? statusQuery.data.error
        : statusQuery.error instanceof Error
          ? statusQuery.error.message
          : null),
    open,
    request,
    reset,
  };
}
