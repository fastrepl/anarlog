import { useQuery } from "@tanstack/react-query";
import { Facehash } from "facehash";
import {
  CalendarIcon,
  ChevronUpIcon,
  CircleHelp,
  FolderOpenIcon,
  SearchIcon,
  SettingsIcon,
  UsersIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useResizeObserver } from "usehooks-ts";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { AuthSection } from "./auth";
import { NotificationsMenuContent } from "./notification";
import { MenuItem } from "./shared";

import { useAuth } from "~/auth";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

type ProfileView = "main" | "notifications";

type ProfileSectionProps = {
  onExpandChange?: (expanded: boolean) => void;
};

export function ProfileSection({ onExpandChange }: ProfileSectionProps = {}) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [currentView, setCurrentView] = useState<ProfileView>("main");
  const [mainViewHeight, setMainViewHeight] = useState<number | null>(null);
  const mainViewRef = useRef<HTMLDivElement | null>(null);
  const openNew = useTabs((state) => state.openNew);
  const transitionChatMode = useTabs((state) => state.transitionChatMode);
  const auth = useAuth();

  const isAuthenticated = !!auth?.session;

  const closeMenu = useCallback(() => {
    setIsExpanded(false);
  }, []);

  useEffect(() => {
    onExpandChange?.(isExpanded);
  }, [isExpanded, onExpandChange]);

  useEffect(() => {
    if (!isExpanded && currentView !== "main") {
      const timer = setTimeout(() => {
        setCurrentView("main");
      }, 300);
      return () => clearTimeout(timer);
    }
  }, [isExpanded, currentView]);

  useEffect(() => {
    if (!isExpanded) {
      setMainViewHeight(null);
    }
  }, [isExpanded]);

  const handleMainViewResize = useCallback(
    ({ height }: { width?: number; height?: number }) => {
      if (!isExpanded || currentView !== "main") {
        return;
      }
      if (height && height > 0) {
        setMainViewHeight(height);
      }
    },
    [isExpanded, currentView],
  );

  useResizeObserver({
    ref: mainViewRef as React.RefObject<HTMLDivElement>,
    onResize: handleMainViewResize,
  });

  const profileRef = useAutoCloser(closeMenu, {
    esc: isExpanded,
    outside: isExpanded,
  });

  const handleClickSettings = useCallback(() => {
    openNew({ type: "settings" });
    closeMenu();
  }, [openNew, closeMenu]);

  const handleClickFolders = useCallback(() => {
    openNew({ type: "folders", id: null });
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

  // const handleClickNotifications = useCallback(() => {
  //   setCurrentView("notifications");
  // }, []);

  const handleBackToMain = useCallback(() => {
    setCurrentView("main");
  }, []);

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
    closeMenu();
  }, [openNew, transitionChatMode, closeMenu]);

  const handleClickAdvancedSearch = useCallback(() => {
    openNew({ type: "search" });
    closeMenu();
  }, [openNew, closeMenu]);

  // const handleClickData = useCallback(() => {
  //   openNew({ type: "data" });
  //   closeMenu();
  // }, [openNew, closeMenu]);

  const kbdClass = cn([
    "transition-all duration-100",
    "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
    "group-active:translate-y-0.5 group-active:shadow-none",
  ]);

  const menuItems = [
    {
      icon: FolderOpenIcon,
      label: "Folders",
      onClick: handleClickFolders,
      badge: <Kbd className={kbdClass}>⌘ ⇧ L</Kbd>,
    },
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
      icon: SearchIcon,
      label: "Advanced Search",
      onClick: handleClickAdvancedSearch,
      badge: <Kbd className={kbdClass}>⌘ ⇧ F</Kbd>,
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
    <div ref={profileRef} className="relative">
      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="absolute right-0 bottom-full left-0 mb-1"
          >
            <div className="overflow-hidden rounded-xl border bg-white shadow-xs">
              <div className="py-1">
                <AnimatePresence mode="wait">
                  {currentView === "main" ? (
                    <motion.div
                      key="main"
                      initial={{ x: 0, opacity: 0 }}
                      animate={{ x: 0, opacity: 1 }}
                      exit={{ x: 0, opacity: 0 }}
                      transition={{
                        duration: 0.2,
                        ease: "easeInOut",
                      }}
                      ref={mainViewRef}
                    >
                      {/*<NotificationsMenuHeader
                        onClick={handleClickNotifications}
                      />*/}

                      {menuItems.map((item, index) => (
                        <div key={item.label}>
                          <MenuItem {...item} />
                          {(index === 3 || index === 5) && (
                            <div className="my-1 border-t border-neutral-100" />
                          )}
                        </div>
                      ))}

                      <AuthSection
                        isAuthenticated={isAuthenticated}
                        onClose={closeMenu}
                      />
                    </motion.div>
                  ) : (
                    <motion.div
                      key="notifications"
                      initial={{ x: 20, opacity: 0 }}
                      animate={{ x: 0, opacity: 1 }}
                      exit={{ x: 20, opacity: 0 }}
                      transition={{
                        duration: 0.2,
                        ease: "easeInOut",
                      }}
                      style={
                        mainViewHeight ? { height: mainViewHeight } : undefined
                      }
                    >
                      <NotificationsMenuContent onBack={handleBackToMain} />
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="overflow-hidden">
        <ProfileButton
          isExpanded={isExpanded}
          onClick={() => setIsExpanded(!isExpanded)}
        />
      </div>
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
  const [imgError, setImgError] = useState(false);

  const profile = useQuery({
    queryKey: ["profile"],
    queryFn: async () => {
      const avatarUrl = await auth?.getAvatarUrl();
      return avatarUrl;
    },
  });

  const facehashName = useMemo(
    () => auth?.session?.user.email || name || "user",
    [auth?.session?.user.email, name],
  );

  useEffect(() => {
    setImgError(false);
  }, [profile.data]);

  const showFacehash = !profile.data || imgError;

  return (
    <button
      className={cn([
        "flex w-full cursor-pointer items-center gap-2.5 rounded-lg",
        "px-4 py-2",
        "text-left",
        "transition-all duration-300",
        "hover:bg-neutral-200/50",
        isExpanded && "border-neutral-300 bg-neutral-200/50",
      ])}
      onClick={onClick}
    >
      <div
        className={cn([
          "flex size-8 shrink-0 items-center justify-center",
          "overflow-hidden rounded-full",
          "shadow-xs",
          "transition-transform duration-300",
        ])}
      >
        {showFacehash ? (
          <div className="rounded-full bg-amber-50">
            <Facehash
              name={facehashName}
              size={32}
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
      <div className="min-w-0 flex-1 truncate text-sm text-black">{name}</div>
      <ChevronUpIcon
        className={cn([
          "h-4 w-4",
          "transition-transform duration-300",
          isExpanded ? "rotate-180 text-neutral-500" : "text-neutral-400",
        ])}
      />
    </button>
  );
}

function useMyName(email?: string) {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);
  return name || email || "Unknown";
}
