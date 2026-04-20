import {
  CalendarIcon,
  CircleHelp,
  FolderOpenIcon,
  SettingsIcon,
  UsersIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

type MenuItem = {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  badge?: React.ReactNode;
  onClick: () => void;
};

export function ProfileMenuView({
  isOpen,
  displayName,
  plan,
  onToggle,
  onSettings,
  onFolders,
  onContacts,
  onCalendar,
  onHelp,
}: {
  isOpen: boolean;
  displayName: string;
  plan: string;
  onToggle: () => void;
  onSettings: () => void;
  onFolders: () => void;
  onContacts: () => void;
  onCalendar: () => void;
  onHelp: () => void;
}) {
  const kbdClass = cn([
    "transition-all duration-100",
    "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
    "group-active:translate-y-0.5 group-active:shadow-none",
  ]);

  const menuItems: MenuItem[] = [
    {
      icon: FolderOpenIcon,
      label: "Folders",
      onClick: onFolders,
      badge: <Kbd className={kbdClass}>⌘ ⇧ L</Kbd>,
    },
    {
      icon: UsersIcon,
      label: "Contacts",
      onClick: onContacts,
      badge: <Kbd className={kbdClass}>⌘ ⇧ O</Kbd>,
    },
    {
      icon: CalendarIcon,
      label: "Calendar",
      onClick: onCalendar,
      badge: <Kbd className={kbdClass}>⌘ ⇧ C</Kbd>,
    },
    {
      icon: SettingsIcon,
      label: "Settings",
      onClick: onSettings,
      badge: <Kbd className={kbdClass}>⌘ ,</Kbd>,
    },
    {
      icon: CircleHelp,
      label: "Help",
      onClick: onHelp,
    },
  ];

  return (
    <div className="relative">
      <AvatarButton
        displayName={displayName}
        isOpen={isOpen}
        onClick={onToggle}
        plan={plan}
      />
      <AnimatePresence>
        {isOpen ? (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15, ease: "easeInOut" }}
            className="absolute top-full right-0 z-50 mt-1"
          >
            <div className="w-56 overflow-hidden rounded-xl border bg-white shadow-xs">
              <div className="py-1">
                {menuItems.map((item) => (
                  <MenuItemView key={item.label} {...item} />
                ))}
                <ProfileName displayName={displayName} plan={plan} />
              </div>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function MenuItemView({ icon: Icon, label, badge, onClick }: MenuItem) {
  return (
    <div className="px-1">
      <button
        type="button"
        className={cn([
          "group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg",
          "px-3 py-1.5",
          "text-sm whitespace-nowrap text-black",
          "transition-colors hover:bg-neutral-100",
        ])}
        onClick={onClick}
      >
        <div className="flex items-center justify-start gap-2.5">
          <Icon className="h-4 w-4 shrink-0 text-black" />
          {label}
        </div>
        {badge ?? null}
      </button>
    </div>
  );
}

function ProfileName({
  displayName,
  plan,
}: {
  displayName: string;
  plan: string;
}) {
  const badgeLabel = plan.toUpperCase();

  return (
    <div className="flex items-center gap-2.5 px-3 py-2">
      <div className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-full border border-neutral-300 bg-amber-50 text-xs font-semibold text-amber-700">
        {displayName.slice(0, 1).toUpperCase()}
      </div>
      <div className="min-w-0 flex-1 truncate text-sm text-black">
        {displayName}
      </div>
      <span
        className={cn([
          "shrink-0 rounded px-1.5 py-0.5 text-[10px] leading-none font-semibold",
          plan === "pro"
            ? "bg-amber-100 text-amber-700"
            : "bg-neutral-100 text-neutral-500",
        ])}
      >
        {badgeLabel}
      </span>
    </div>
  );
}

function AvatarButton({
  displayName,
  isOpen,
  onClick,
  plan,
}: {
  displayName: string;
  isOpen: boolean;
  onClick: () => void;
  plan: string;
}) {
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
      title={`${displayName} (${plan})`}
    >
      <div
        className={cn([
          "flex size-5 shrink-0 items-center justify-center",
          "overflow-hidden rounded-full border border-neutral-300 bg-amber-50",
          "text-[10px] font-semibold text-amber-700",
        ])}
      >
        {displayName.slice(0, 1).toUpperCase()}
      </div>
    </button>
  );
}
