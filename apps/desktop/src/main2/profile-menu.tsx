import { useQuery } from "@tanstack/react-query";
import { Facehash } from "facehash";
import {
  CalendarIcon,
  CircleHelp,
  FolderOpenIcon,
  SettingsIcon,
  UsersIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import { AuthSection } from "~/sidebar/profile/auth";
import { MenuItem } from "~/sidebar/profile/shared";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function ProfileMenu() {
  const [isOpen, setIsOpen] = useState(false);
  const openNew = useTabs((state) => state.openNew);
  const transitionChatMode = useTabs((state) => state.transitionChatMode);
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const isAuthenticated = !!auth?.session;

  const close = useCallback(() => setIsOpen(false), []);

  const ref = useAutoCloser(close, {
    esc: isOpen,
    outside: isOpen,
  });

  const handleClickSettings = useCallback(() => {
    openNew({ type: "settings" });
    close();
  }, [openNew, close]);

  const handleClickFolders = useCallback(() => {
    openNew({ type: "folders", id: null });
    close();
  }, [openNew, close]);

  const handleClickCalendar = useCallback(() => {
    openNew({ type: "calendar" });
    close();
  }, [openNew, close]);

  const handleClickContacts = useCallback(() => {
    openNew({ type: "contacts", state: { selected: null } });
    close();
  }, [openNew, close]);

  const handleClickHelp = useCallback(() => {
    const state = {
      groupId: null,
      initialMessage: "I need help.",
    };
    openNew({ type: "chat_support", state });
    const { tabs, updateChatSupportTabState } = useTabs.getState();
    const existingChatTab = tabs.find((t) => t.type === "chat_support");
    if (existingChatTab) {
      updateChatSupportTabState(existingChatTab, state);
    }
    transitionChatMode({ type: "OPEN_TAB" });
    close();
  }, [openNew, transitionChatMode, close]);

  const kbdClass = cn([
    "transition-all duration-100",
    "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
    "group-active:translate-y-0.5 group-active:shadow-none",
  ]);

  const menuItems = [
    ...(isPro
      ? [
          {
            icon: FolderOpenIcon,
            label: "Folders",
            onClick: handleClickFolders,
            badge: <Kbd className={kbdClass}>⌘ ⇧ L</Kbd>,
          },
        ]
      : []),
    {
      icon: UsersIcon,
      label: "Contacts",
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
    {
      icon: CircleHelp,
      label: "Help",
      onClick: handleClickHelp,
    },
  ];

  return (
    <div ref={ref} className="relative">
      <AvatarButton isOpen={isOpen} onClick={() => setIsOpen(!isOpen)} />
      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15, ease: "easeInOut" }}
            className="absolute top-full right-0 z-50 mt-1"
          >
            <div className="w-56 overflow-hidden rounded-xl border bg-white shadow-xs">
              <div className="py-1">
                {menuItems.map((item, index) => (
                  <div key={item.label}>
                    <MenuItem {...item} />
                    {index === (isPro ? 2 : 1) && (
                      <div className="my-1 border-t border-neutral-100" />
                    )}
                  </div>
                ))}
                <AuthSection
                  isAuthenticated={isAuthenticated}
                  onClose={close}
                />
                {isAuthenticated && <ProfileName />}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ProfileName() {
  const auth = useAuth();
  const { plan } = useBillingAccess();
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);
  const displayName = name || auth?.session?.user.email || "Unknown";

  const facehashName = useMemo(
    () => auth?.session?.user.email || displayName || "user",
    [auth?.session?.user.email, displayName],
  );

  const badgeLabel = plan === "trial" ? "PRO" : plan.toUpperCase();

  return (
    <div className="flex items-center gap-2.5 px-3 py-2">
      <div className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-full border border-neutral-300">
        <div className="rounded-full bg-amber-50">
          <Facehash
            name={facehashName}
            size={24}
            interactive={false}
            showInitial={false}
          />
        </div>
      </div>
      <div className="min-w-0 flex-1 truncate text-sm text-black">
        {displayName}
      </div>
      <span
        className={cn([
          "shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-none font-semibold",
          plan === "pro" || plan === "trial"
            ? "bg-amber-100 text-amber-700"
            : plan === "lite"
              ? "bg-blue-100 text-blue-700"
              : "bg-neutral-100 text-neutral-500",
        ])}
      >
        {badgeLabel}
      </span>
    </div>
  );
}

function AvatarButton({
  isOpen,
  onClick,
}: {
  isOpen: boolean;
  onClick: () => void;
}) {
  const auth = useAuth();
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);
  const displayName = name || auth?.session?.user.email || "Unknown";
  const [imgError, setImgError] = useState(false);

  const profile = useQuery({
    queryKey: ["profile"],
    queryFn: async () => {
      const avatarUrl = await auth?.getAvatarUrl();
      return avatarUrl;
    },
  });

  const facehashName = useMemo(
    () => auth?.session?.user.email || displayName || "user",
    [auth?.session?.user.email, displayName],
  );

  useEffect(() => {
    setImgError(false);
  }, [profile.data]);

  const showFacehash = !profile.data || imgError;

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "flex size-8 cursor-pointer items-center justify-center rounded-md",
        "transition-colors duration-150",
        "hover:bg-neutral-100",
        isOpen && "bg-neutral-200 hover:bg-neutral-200",
      ])}
      title={displayName}
    >
      <div
        className={cn([
          "flex size-5 shrink-0 items-center justify-center",
          "overflow-hidden rounded-full",
          "border border-neutral-300",
        ])}
      >
        {showFacehash ? (
          <div className="rounded-full bg-amber-50">
            <Facehash
              name={facehashName}
              size={20}
              interactive={false}
              showInitial={false}
            />
          </div>
        ) : (
          <img
            src={profile.data!}
            alt="Profile"
            className="h-full w-full rounded-full"
            onError={() => setImgError(true)}
          />
        )}
      </div>
    </button>
  );
}
