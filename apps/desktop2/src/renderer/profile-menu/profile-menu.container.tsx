import { useEffect, useRef, useState } from "react";

import { hypr } from "~/bridge";
import { ProfileMenuView } from "~/profile-menu/profile-menu.view";
import { useTabsStore } from "~/tabs";

export function ProfileMenuContainer() {
  const ref = useRef<HTMLDivElement>(null);
  const [isOpen, setIsOpen] = useState(false);
  const openNew = useTabsStore((state) => state.openNew);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleOpenStub = (
    type: "settings" | "folders" | "contacts" | "calendar",
  ) => {
    openNew({ type });
    setIsOpen(false);
  };

  return (
    <div ref={ref}>
      <ProfileMenuView
        isOpen={isOpen}
        displayName="You"
        plan="free"
        onToggle={() => setIsOpen((current) => !current)}
        onSettings={() => handleOpenStub("settings")}
        onFolders={() => handleOpenStub("folders")}
        onContacts={() => handleOpenStub("contacts")}
        onCalendar={() => handleOpenStub("calendar")}
        onHelp={() => {
          void hypr.openExternal("https://char.com/discord");
          setIsOpen(false);
        }}
      />
    </div>
  );
}
