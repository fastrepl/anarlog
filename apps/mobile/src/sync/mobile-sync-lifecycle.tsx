import { useMountEffect } from "@/lib/use-mount-effect";
import { activateMobileSync } from "@/sync/mobile-sync";

export function MobileSyncLifecycle({
  accessToken,
  accountUserId,
}: {
  accessToken: string;
  accountUserId: string;
}) {
  useMountEffect(() => activateMobileSync({ accessToken, accountUserId }));
  return null;
}
