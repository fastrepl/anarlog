import { useQuery } from "@tanstack/react-query";
import { CalendarIcon, SettingsIcon, UsersIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useState } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { AuthSection } from "./auth";
import { MenuItem, ProfileFacehash } from "./shared";

import { useAuth } from "~/auth";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function ProfileMenu() {
  const [isExpanded, setIsExpanded] = useState(false);
  const openNew = useTabs((state) => state.openNew);
  const auth = useAuth();

  const isAuthenticated = !!auth?.session;

  const closeMenu = useCallback(() => {
    setIsExpanded(false);
  }, []);

  const profileRef = useAutoCloser(closeMenu, {
    esc: isExpanded,
    outside: isExpanded,
  });

  const handleClickSettings = useCallback(() => {
    openNew({ type: "settings" });
    closeMenu();
  }, [openNew, closeMenu]);

  const handleClickCalendar = useCallback(() => {
    openNew({ type: "calendar" });
    closeMenu();
  }, [openNew, closeMenu]);

  const handleClickContacts = useCallback(() => {
    openNew({
      type: "contacts",
      state: {
        selected: null,
      },
    });
    closeMenu();
  }, [openNew, closeMenu]);

  const kbdClass = cn([
    "transition-all duration-100",
    "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
    "group-active:translate-y-0.5 group-active:shadow-none",
  ]);

  const menuItems = [
    {
      icon: UsersIcon,
      label: "People",
      onClick: handleClickContacts,
      badge: <Kbd className={kbdClass}>⌘ ⇧ O</Kbd>,
    },
    {
      icon: CalendarIcon,
      label: "Calendar",
      onClick: handleClickCalendar,
      badge: <Kbd className={kbdClass}>⌘ ⇧ C</Kbd>,
    },
    {
      icon: SettingsIcon,
      label: "Settings",
      onClick: handleClickSettings,
      badge: <Kbd className={kbdClass}>⌘ ,</Kbd>,
    },
  ];

  return (
    <div
      ref={profileRef}
      className="relative z-50 mr-1 flex h-full shrink-0 items-center"
      data-tauri-drag-region="false"
    >
      <ProfileButton
        isExpanded={isExpanded}
        onClick={() => setIsExpanded((expanded) => !expanded)}
      />

      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.16, ease: "easeInOut" }}
            className="absolute top-full left-0 mt-1 w-56"
            data-tauri-drag-region="false"
          >
            <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white shadow-[0_10px_30px_rgba(0,0,0,0.14)]">
              <div className="py-1">
                {menuItems.map((item) => (
                  <MenuItem key={item.label} {...item} />
                ))}

                <AuthSection
                  isAuthenticated={isAuthenticated}
                  onClose={closeMenu}
                />
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ProfileButton({
  isExpanded,
  onClick,
}: {
  isExpanded: boolean;
  onClick: () => void;
}) {
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
      aria-label="Open profile menu"
      aria-expanded={isExpanded}
      className={cn([
        "flex size-8 cursor-pointer items-center justify-center rounded-lg",
        "bg-neutral-200/70 p-1 shadow-xs ring-1 ring-neutral-200",
        "transition-colors duration-150",
        "hover:bg-neutral-300/70",
        isExpanded && "bg-neutral-300/70",
      ])}
      onClick={onClick}
    >
      <div
        className={cn([
          "flex size-6 shrink-0 items-center justify-center",
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
            size={24}
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
