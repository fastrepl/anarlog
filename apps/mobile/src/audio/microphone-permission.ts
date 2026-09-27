import { useMutation, useQuery } from "@tanstack/react-query";
import {
  getRecordingPermissionsAsync,
  requestRecordingPermissionsAsync,
} from "expo-audio";
import { Linking } from "react-native";

export function useMicrophonePermission() {
  const permission = useQuery({
    queryKey: ["microphone-permission"],
    queryFn: getRecordingPermissionsAsync,
    refetchInterval: 2000,
  });
  const request = useMutation({
    mutationFn: async () => {
      if (permission.data?.canAskAgain && !permission.data.granted)
        await requestRecordingPermissionsAsync();
      else await Linking.openSettings();
      await permission.refetch();
    },
  });
  return { permission, request };
}
