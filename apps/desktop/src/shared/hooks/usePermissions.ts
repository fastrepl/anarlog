import { useMutation, useQuery } from "@tanstack/react-query";
import { message } from "@tauri-apps/plugin-dialog";

import {
  type Permission,
  commands as permissionsCommands,
  type PermissionStatus,
} from "@hypr/plugin-permissions";

import { relaunch } from "~/store/tinybase/store/save";

export function usePermission(type: Permission) {
  const status = useQuery({
    queryKey: [`${type}Permission`],
    queryFn: () => permissionsCommands.checkPermission(type),
    refetchInterval: 1000,
    select: (result): PermissionStatus => {
      if (result.status === "error") {
        return "denied";
      }
      return result.data;
    },
  });

  const requestMutation = useMutation({
    mutationFn: () => permissionsCommands.requestPermission(type),
    onSuccess: () => {
      if (type === "systemAudio") {
        void message("The app will now restart to apply the changes", {
          kind: "info",
          title: "System Audio Status Changed",
        });
        setTimeout(() => relaunch(), 2000);
        return;
      }
      setTimeout(() => status.refetch(), 1000);
    },
  });

  const resetMutation = useMutation({
    mutationFn: () => permissionsCommands.resetPermission(type),
    onSuccess: () => {
      setTimeout(() => status.refetch(), 1000);
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

  return { status: status.data, isPending, open, request, reset };
}

export function usePermissions() {
  const micPermissionStatus = useQuery({
    queryKey: ["micPermission"],
    queryFn: () => permissionsCommands.checkPermission("microphone"),
    refetchInterval: 1000,
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
    refetchInterval: 1000,
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
    refetchInterval: 1000,
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
      setTimeout(() => {
        void micPermissionStatus.refetch();
      }, 1000);
    },
    onError: (error) => {
      console.error(error);
    },
  });

  const systemAudioPermission = useMutation({
    mutationFn: () => permissionsCommands.requestPermission("systemAudio"),
    onSuccess: () => {
      void message("The app will now restart to apply the changes", {
        kind: "info",
        title: "System Audio Status Changed",
      });
      setTimeout(() => relaunch(), 2000);
    },
    onError: console.error,
  });

  const accessibilityPermission = useMutation({
    mutationFn: () => permissionsCommands.requestPermission("accessibility"),
    onSuccess: () => {
      setTimeout(() => {
        void accessibilityPermissionStatus.refetch();
      }, 1000);
    },
    onError: console.error,
  });

  const micResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("microphone"),
    onSuccess: () => {
      setTimeout(() => {
        void micPermissionStatus.refetch();
      }, 1000);
    },
    onError: console.error,
  });

  const systemAudioResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("systemAudio"),
    onSuccess: () => {
      setTimeout(() => {
        void systemAudioPermissionStatus.refetch();
      }, 1000);
    },
    onError: console.error,
  });

  const accessibilityResetPermission = useMutation({
    mutationFn: () => permissionsCommands.resetPermission("accessibility"),
    onSuccess: () => {
      setTimeout(() => {
        void accessibilityPermissionStatus.refetch();
      }, 1000);
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
