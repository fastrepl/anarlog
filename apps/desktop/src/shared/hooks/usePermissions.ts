import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
  type Permission,
  commands as permissionsCommands,
  type PermissionStatus,
} from "@hypr/plugin-permissions";

export function usePermission(type: Permission) {
  const [optimisticStatus, setOptimisticStatus] =
    useState<PermissionStatus | null>(null);
  const status = useQuery({
    queryKey: [`${type}Permission`],
    queryFn: () => permissionsCommands.checkPermission(type),
    select: (result): PermissionStatus => {
      if (result.status === "error") {
        return "denied";
      }
      return result.data;
    },
  });

  const requestMutation = useMutation({
    mutationFn: () => permissionsCommands.requestPermission(type),
    onSuccess: async () => {
      if (type === "systemAudio") {
        setOptimisticStatus("authorized");
        schedulePermissionRefetch(status.refetch);
        return;
      }
      setOptimisticStatus(null);
      schedulePermissionRefetch(status.refetch);
    },
  });

  const resetMutation = useMutation({
    mutationFn: () => permissionsCommands.resetPermission(type),
    onSuccess: () => {
      setOptimisticStatus(null);
      schedulePermissionRefetch(status.refetch);
    },
  });

  const isPending = requestMutation.isPending || resetMutation.isPending;

  const open = async () => {
    await permissionsCommands.openPermission(type);
  };

  const request = () => {
    requestMutation.mutate();
  };

  const reset = () => {
    resetMutation.mutate();
  };

  useEffect(() => {
    const onWindowFocus = () => {
      void status.refetch();
    };

    window.addEventListener("focus", onWindowFocus);
    return () => {
      window.removeEventListener("focus", onWindowFocus);
    };
  }, [status]);

  return {
    status: optimisticStatus ?? status.data,
    isPending,
    open,
    request,
    reset,
  };
}

export function usePermissions() {
  const micPermissionStatus = useQuery({
    queryKey: ["micPermission"],
    queryFn: () => permissionsCommands.checkPermission("microphone"),
    select: (result) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const systemAudioPermissionStatus = useQuery({
    queryKey: ["systemAudioPermission"],
    queryFn: () => permissionsCommands.checkPermission("systemAudio"),
    select: (result) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const accessibilityPermissionStatus = useQuery({
    queryKey: ["accessibilityPermission"],
    queryFn: () => permissionsCommands.checkPermission("accessibility"),
    select: (result) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const micPermission = useMutation({
    mutationFn: () => permissionsCommands.requestPermission("microphone"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void micPermissionStatus.refetch();
      });
    },
    onError: (error) => {
      console.error(error);
    },
  });

  const systemAudioPermission = useMutation({
    mutationFn: () => permissionsCommands.requestPermission("systemAudio"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void systemAudioPermissionStatus.refetch();
      });
    },
    onError: console.error,
  });

  const accessibilityPermission = useMutation({
    mutationFn: () => permissionsCommands.requestPermission("accessibility"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void accessibilityPermissionStatus.refetch();
      });
    },
    onError: console.error,
  });

  const micResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("microphone"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void micPermissionStatus.refetch();
      });
    },
    onError: console.error,
  });

  const systemAudioResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("systemAudio"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void systemAudioPermissionStatus.refetch();
      });
    },
    onError: console.error,
  });

  const accessibilityResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("accessibility"),
    onSuccess: () => {
      schedulePermissionRefetch(() => {
        void accessibilityPermissionStatus.refetch();
      });
    },
    onError: console.error,
  });

  const openMicrophoneSettings = async () => {
    await permissionsCommands.openPermission("microphone");
  };

  const openSystemAudioSettings = async () => {
    await permissionsCommands.openPermission("systemAudio");
  };

  const openAccessibilitySettings = async () => {
    await permissionsCommands.openPermission("accessibility");
  };

  const handleMicPermissionAction = async () => {
    if (micPermissionStatus.data === "denied") {
      await openMicrophoneSettings();
    } else {
      micPermission.mutate(undefined);
    }
  };

  const handleSystemAudioPermissionAction = async () => {
    if (systemAudioPermissionStatus.data === "denied") {
      await openSystemAudioSettings();
    } else {
      systemAudioPermission.mutate(undefined);
    }
  };

  const handleAccessibilityPermissionAction = async () => {
    if (accessibilityPermissionStatus.data === "denied") {
      await openAccessibilitySettings();
    } else {
      accessibilityPermission.mutate(undefined);
    }
  };

  useEffect(() => {
    const onWindowFocus = () => {
      void micPermissionStatus.refetch();
      void systemAudioPermissionStatus.refetch();
      void accessibilityPermissionStatus.refetch();
    };

    window.addEventListener("focus", onWindowFocus);
    return () => {
      window.removeEventListener("focus", onWindowFocus);
    };
  }, [
    micPermissionStatus,
    systemAudioPermissionStatus,
    accessibilityPermissionStatus,
  ]);

  return {
    micPermissionStatus,
    systemAudioPermissionStatus,
    accessibilityPermissionStatus,
    micPermission,
    systemAudioPermission,
    accessibilityPermission,
    micResetPermission,
    systemAudioResetPermission,
    accessibilityResetPermission,
    openMicrophoneSettings,
    openSystemAudioSettings,
    openAccessibilitySettings,
    handleMicPermissionAction,
    handleSystemAudioPermissionAction,
    handleAccessibilityPermissionAction,
  };
}

function schedulePermissionRefetch(refetch: () => void | Promise<unknown>) {
  window.setTimeout(() => {
    void refetch();
  }, 1000);
}
