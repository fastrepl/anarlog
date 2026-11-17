import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Platform } from "react-native";

import { useMountEffect } from "@/lib/use-mount-effect";
import { readProviderConfig } from "@/settings/providers";

import { startLocalTranscriptionRecovery } from "./local-transcription-recovery";

export function LocalTranscriptionLifecycle({
  accountUserId,
}: {
  accountUserId: string | null;
}) {
  const config = useQuery({
    queryKey: ["provider", accountUserId, "stt"],
    queryFn: () => readProviderConfig(accountUserId, "stt"),
    enabled: Platform.OS === "android",
  });
  return Platform.OS === "android" &&
    config.data?.provider === "google_on_device" ? (
    <LocalRecovery
      key={`${accountUserId}:${config.data.model}`}
      accountUserId={accountUserId}
    />
  ) : null;
}

function LocalRecovery({ accountUserId }: { accountUserId: string | null }) {
  const queryClient = useQueryClient();
  useMountEffect(() =>
    startLocalTranscriptionRecovery(accountUserId, queryClient),
  );
  return null;
}
