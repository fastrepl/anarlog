import { useEffect, useState } from "react";

import { cn } from "@anlg/utils";

export const ACCOUNT_SECTIONS = [
  { id: "profile", label: "Profile" },
  { id: "referrals", label: "Refer friends" },
  { id: "plan", label: "Your plan" },
  { id: "integrations", label: "Integrations" },
  { id: "devices", label: "Synced devices" },
  { id: "shares", label: "Shared notes" },
  { id: "api-keys", label: "Cloud API keys" },
  { id: "session", label: "Session controls" },
  { id: "danger", label: "Danger area" },
] as const;

export function useActiveAccountSection() {
  const [activeId, setActiveId] = useState<string>(ACCOUNT_SECTIONS[0].id);

  useEffect(() => {
    const elements = ACCOUNT_SECTIONS.map((section) =>
      document.getElementById(section.id),
    ).filter((element): element is HTMLElement => element !== null);

    if (elements.length === 0) {
      return;
    }

    const visibleIds = new Set<string>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            visibleIds.add(entry.target.id);
          } else {
            visibleIds.delete(entry.target.id);
          }
        }

        const next = ACCOUNT_SECTIONS.find((section) =>
          visibleIds.has(section.id),
        );
        if (next) {
          setActiveId(next.id);
        }
      },
      { rootMargin: "-20% 0px -65% 0px", threshold: [0, 0.25, 0.5] },
    );

    for (const element of elements) {
      observer.observe(element);
    }

    return () => observer.disconnect();
  }, []);

  return activeId;
}

export function AccountNav({ activeId }: { activeId: string }) {
  return (
    <nav aria-label="Account sections">
      <ul
        className={cn([
          "flex gap-1 overflow-x-auto px-5",
          "lg:flex-col lg:overflow-visible lg:px-0",
        ])}
      >
        {ACCOUNT_SECTIONS.map((section) => {
          const isActive = section.id === activeId;

          return (
            <li
              key={section.id}
              className={cn([
                "shrink-0",
                section.id === "danger" &&
                  "lg:mt-2 lg:border-t lg:border-[#ede7dc] lg:pt-2",
              ])}
            >
              <a
                href={`#${section.id}`}
                aria-current={isActive ? "true" : undefined}
                className={cn([
                  "block rounded-full px-3 py-1.5 text-sm whitespace-nowrap transition-colors",
                  isActive
                    ? "bg-[#fff0b3] font-medium text-[#181613]"
                    : "text-[#756b5d] hover:text-[#181613]",
                  !isActive && section.id === "danger" && "hover:text-red-700",
                ])}
              >
                {section.label}
              </a>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
