import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { cn } from "@hypr/utils";

import { ProfileFacehash } from "./shared";

import { useAuth } from "~/auth";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function ProfileMenu() {
  const openNew = useTabs((state) => state.openNew);

  const handleClickSettings = useCallback(() => {
    openNew({ type: "settings" });
  }, [openNew]);

  return (
    <div
      className="relative z-50 mr-1 flex h-full shrink-0 items-center"
      data-tauri-drag-region="false"
    >
      <ProfileButton onClick={handleClickSettings} />
    </div>
  );
}

function ProfileButton({ onClick }: { onClick: () => void }) {
  const auth = useAuth();
  const name = useMyName(auth?.session?.user.email);
  const [failedAvatarUrl, setFailedAvatarUrl] = useState<string | null>(null);

  const profile = useQuery({
    queryKey: ["profile"],
    queryFn: async () => {
      const avatarUrl = await auth?.getAvatarUrl();
      return avatarUrl;
    },
  });

  const facehashName = name;
  const avatarUrl = profile.data ?? null;
  const validAvatarUrl =
    avatarUrl && failedAvatarUrl !== avatarUrl ? avatarUrl : null;

  return (
    <button
      type="button"
      data-tauri-drag-region="false"
      aria-label="Open settings"
      className={cn([
        "flex size-7 cursor-pointer items-center justify-center rounded-lg",
        "border border-transparent bg-transparent p-1",
        "transition-colors duration-150",
        "hover:border-neutral-200 hover:bg-neutral-200/70",
      ])}
      onClick={onClick}
    >
      <div
        className={cn([
          "flex size-[18px] shrink-0 items-center justify-center",
          "overflow-hidden rounded-md",
          "shadow-xs",
          "transition-transform duration-300",
        ])}
      >
        {validAvatarUrl ? (
          <img
            key={validAvatarUrl}
            src={validAvatarUrl}
            alt="Profile"
            className="h-full w-full rounded-md"
            onError={() => setFailedAvatarUrl(validAvatarUrl)}
          />
        ) : (
          <ProfileFacehash
            name={facehashName}
            size={18}
            className="rounded-md"
          />
        )}
      </div>
    </button>
  );
}

function useMyName(email?: string) {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);
  return name || email || "Unknown";
}
