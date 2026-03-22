import { Link, useRouterState } from "@tanstack/react-router";
import {
  BookOpen,
  Building2,
  ChevronRight,
  FileText,
  History,
  LayoutTemplate,
  Map,
  Menu,
  MessageCircle,
  Newspaper,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import {
  AnimatePresence,
  motion,
  useMotionValueEvent,
  useScroll,
} from "motion/react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { SearchTrigger } from "@/components/search";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

const featuresList = [
  { to: "/product/ai-notetaking", label: "AI Notetaking" },
  { to: "/product/search", label: "Searchable Notes" },
  { to: "/gallery?type=template", label: "Custom Templates" },
  { to: "/product/markdown", label: "Markdown Files" },
  { to: "/product/flexible-ai", label: "Flexible AI" },
  { to: "/opensource", label: "Open Source" },
];

const solutionsList = [
  { to: "/solution/knowledge-workers", label: "For Knowledge Workers" },
  { to: "/enterprise", label: "For Enterprises" },
  { to: "/product/api", label: "For Developers" },
];

const resourcesList: {
  to: string;
  label: string;
  icon: LucideIcon;
  external?: boolean;
}[] = [
  { to: "/blog/", label: "Blog", icon: FileText },
  { to: "/docs/", label: "Documentation", icon: BookOpen },
  {
    to: "/gallery?type=template",
    label: "Meeting Templates",
    icon: LayoutTemplate,
  },
  { to: "/updates/", label: "Updates", icon: Newspaper },
  { to: "/changelog/", label: "Changelog", icon: History },
  { to: "/roadmap/", label: "Roadmap", icon: Map },
  { to: "/company-handbook/", label: "Company Handbook", icon: Building2 },
  {
    to: "https://discord.gg/hyprnote",
    label: "Community",
    icon: MessageCircle,
    external: true,
  },
];

const homeSections = [
  { id: "hero", label: "Intro" },
  { id: "how-it-works", label: "How it works" },
  { id: "ai", label: "AI features" },
  { id: "grows-with-you", label: "Grows with you" },
  { id: "solutions", label: "Solutions" },
  { id: "faq", label: "FAQ" },
  { id: "blog", label: "Blog" },
];

const homeSectionIds = homeSections.map((s) => s.id);

function useActiveHomeSection(enabled: boolean) {
  const [activeId, setActiveId] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled) {
      setActiveId(null);
      return;
    }

    setActiveId(homeSectionIds[0]);

    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.find((e) => e.isIntersecting);
        if (visible) {
          setActiveId(visible.target.id);
        }
      },
      { rootMargin: "-20% 0px -60% 0px", threshold: 0 },
    );

    homeSectionIds.forEach((id) => {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    });

    return () => observer.disconnect();
  }, [enabled]);

  return activeId;
}

function findActiveSubItem(pathname: string) {
  const candidates = [
    ...featuresList.map((i) => ({ ...i, parent: "Product" })),
    ...solutionsList.map((i) => ({ ...i, parent: "Product" })),
    ...resourcesList
      .filter((i) => !i.external)
      .map((i) => ({ ...i, parent: "Resources" })),
  ];
  return (
    candidates.find((item) =>
      pathname.startsWith(item.to.replace(/\/$/, "")),
    ) ?? null
  );
}

function CharLogo({ className }: { className?: string }) {
  return (
    <svg
      width="103"
      height="30"
      viewBox="0 0 103 30"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <path
        d="M7.871 4.147C7.871 5.658 7.082 7.039 6.099 8.214C4.65 9.946 3.77 12.161 3.77 14.575C3.77 16.99 4.65 19.205 6.099 20.937C7.082 22.112 7.871 23.493 7.871 25.004V29.151H2.965V24.319C2.965 22.735 2.165 21.249 0.822 20.34L0 19.783V9.235L0.822 8.678C2.165 7.769 2.965 6.284 2.965 4.699V0L7.871 0V4.147Z"
        fill="currentColor"
      />
      <path
        d="M94.746 4.147C94.746 5.658 95.535 7.039 96.519 8.214C97.967 9.946 98.847 12.161 98.847 14.575C98.847 16.99 97.967 19.205 96.519 20.937C95.535 22.112 94.746 23.493 94.746 25.004V29.151H99.653V24.319C99.653 22.735 100.452 21.249 101.795 20.34L102.617 19.783V9.235L101.795 8.678C100.452 7.769 99.653 6.284 99.653 4.699V0L94.746 0V4.147Z"
        fill="currentColor"
      />
      <path
        d="M90.369 4.536H86.669C84.596 4.536 82.721 5.667 81.73 7.429V4.536H73.026V8.029H78.244V20.821H73.026V24.313H90.311V20.821H82.425V12.447C82.425 10.262 84.191 8.494 86.365 8.494H90.369V4.536Z"
        fill="currentColor"
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M60.901 4.071C63.781 4.071 66.142 5.182 67.798 6.995V4.536H71.284V24.313H67.798V21.805C66.128 23.645 63.753 24.778 60.901 24.778C55.064 24.778 51.331 20.074 51.331 14.425C51.331 11.606 52.225 9.021 53.882 7.131C55.546 5.235 57.954 4.071 60.901 4.071ZM61.365 7.912C59.5 7.912 58.023 8.638 57.005 9.793C55.981 10.956 55.396 12.586 55.396 14.425C55.396 18.088 57.776 20.937 61.365 20.937C64.954 20.937 67.334 18.088 67.334 14.425C67.334 12.586 66.749 10.956 65.725 9.793C64.708 8.638 63.231 7.912 61.365 7.912Z"
        fill="currentColor"
      />
      <path
        d="M49.589 12.098C49.589 7.924 46.214 4.536 42.048 4.536H41.195C39.142 4.536 36.977 5.657 35.905 7.463V0H32.188V24.313H36.369V12.447C36.369 11.405 36.912 10.422 37.78 9.684C38.648 8.944 39.793 8.494 40.891 8.494H41.06C43.345 8.494 45.407 10.359 45.407 12.564V24.313H49.589V12.098Z"
        fill="currentColor"
      />
      <path
        d="M26.243 17.328C25.77 19.561 23.754 21.053 20.995 21.053C17.296 21.053 14.852 18.146 14.852 14.425C14.852 12.556 15.453 10.897 16.506 9.713C17.552 8.536 19.074 7.796 20.995 7.796C23.793 7.796 25.772 9.443 26.26 11.533L26.365 11.983H30.559L30.436 11.297C29.689 7.153 26.043 4.071 20.995 4.071C17.864 4.071 15.3 5.224 13.522 7.117C11.749 9.005 10.787 11.595 10.787 14.425C10.787 20.113 14.807 24.778 20.995 24.778C25.907 24.778 29.753 22.074 30.427 17.535L30.527 16.866H26.341L26.243 17.328Z"
        fill="currentColor"
      />
    </svg>
  );
}

const navLinks = [
  { to: "/why-char/", label: "Why Char" },
  { to: "/product/ai-notetaking/", label: "Product", hasSubmenu: true },
  { to: "/docs/", label: "Resources", hasSubmenu: true },
  { to: "/pricing/", label: "Pricing" },
] as const;

export function Sidebar() {
  const [isMobileOpen, setIsMobileOpen] = useState(false);
  const router = useRouterState();
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);
  const pathname = router.location.pathname;
  const isHomePage = pathname === "/";
  const activeSection = useActiveHomeSection(isHomePage);
  const activeSubItem = isHomePage ? null : findActiveSubItem(pathname);

  const { scrollY } = useScroll();
  const [showCTA, setShowCTA] = useState(!isHomePage);

  useMotionValueEvent(scrollY, "change", (latest) => {
    if (!isHomePage) {
      setShowCTA(true);
      return;
    }
    setShowCTA(latest > window.innerHeight);
  });

  useEffect(() => {
    setShowCTA(!isHomePage);
  }, [isHomePage]);

  useEffect(() => {
    setIsMobileOpen(false);
  }, [pathname]);

  useEffect(() => {
    if (isMobileOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [isMobileOpen]);

  return (
    <>
      <MobileTopBar
        isMobileOpen={isMobileOpen}
        setIsMobileOpen={setIsMobileOpen}
      />

      {isMobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-white md:hidden"
          onClick={() => setIsMobileOpen(false)}
        />
      )}

      <aside className="z-10 hidden w-[200px] shrink-0 self-stretch md:block">
        <div className="sticky top-0 flex h-screen flex-col">
          <div className="px-12 pt-16 pb-10">
            <Link to="/">
              <CharLogo className="text-fg h-8 w-auto transition-colors hover:scale-105" />
            </Link>
          </div>

          <AnimatePresence initial={false}>
            {isHomePage && (
              <motion.div
                key="home-section-nav"
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.25, ease: "easeInOut" }}
                className="overflow-hidden"
              >
                <div className="px-12 pb-4">
                  <HomeSectionNav activeId={activeSection} />
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          <nav className="flex flex-col gap-1 px-12 pt-4">
            {navLinks.map((link) =>
              "hasSubmenu" in link && link.hasSubmenu ? (
                <SidebarFlyout
                  key={link.to}
                  label={link.label}
                  to={link.to}
                  isActive={pathname.startsWith(link.to.replace(/\/$/, ""))}
                  activeSubItem={
                    activeSubItem?.parent === link.label ? activeSubItem : null
                  }
                />
              ) : (
                <Link
                  key={link.to}
                  to={link.to}
                  className={cn(
                    ["py-1.5 text-base transition-colors"],
                    [
                      pathname.startsWith(link.to.replace(/\/$/, ""))
                        ? "text-fg -mx-2 rounded-full px-2 underline"
                        : "text-fg hover:underline",
                    ],
                  )}
                >
                  {link.label}
                </Link>
              ),
            )}
          </nav>

          <div className="flex-1" />

          <div className="shrink-0 px-12 pb-8">
            <div className="flex flex-col gap-3">
              <SearchTrigger variant="header" />
              <SidebarCTA platformCTA={platformCTA} visible={showCTA} />
            </div>
          </div>
        </div>
      </aside>

      {/* Mobile slide-out sidebar */}
      <aside
        className={cn(
          [
            "fixed top-14 left-0 z-50 flex h-[calc(100dvh-56px)] w-[200px] flex-col bg-neutral-950 md:hidden",
            "border-r border-neutral-800 transition-transform duration-300",
          ],
          [isMobileOpen ? "translate-x-0" : "-translate-x-full"],
        )}
      >
        <nav className="flex flex-1 flex-col gap-1 overflow-y-auto px-8 pt-6">
          {navLinks.map((link) =>
            "hasSubmenu" in link && link.hasSubmenu ? (
              <MobileSubmenu
                key={link.to}
                label={link.label}
                to={link.to}
                isActive={pathname.startsWith(link.to.replace(/\/$/, ""))}
              />
            ) : (
              <Link
                key={link.to}
                to={link.to}
                className={cn(
                  ["py-2.5 text-base transition-colors"],
                  [
                    pathname.startsWith(link.to.replace(/\/$/, ""))
                      ? "-mx-2 rounded-lg px-2 text-neutral-200"
                      : "text-neutral-500 hover:text-neutral-300",
                  ],
                )}
              >
                {link.label}
              </Link>
            ),
          )}
        </nav>

        <div className="flex flex-col gap-3 px-8 pb-8">
          <SidebarCTA platformCTA={platformCTA} />
        </div>
      </aside>
    </>
  );
}

function HomeSectionNav({ activeId }: { activeId: string | null }) {
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  const scrollTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  };

  return (
    <nav
      className="border-brand-bright flex flex-col gap-1.5 border-b pb-4"
      onMouseLeave={() => setHoveredId(null)}
    >
      {homeSections.map((s) => {
        const showLabel = activeId === s.id || hoveredId === s.id;
        return (
          <button
            key={s.id}
            onClick={() => scrollTo(s.id)}
            onMouseEnter={() => setHoveredId(s.id)}
            className="flex h-5 cursor-pointer items-center text-left"
          >
            {showLabel ? (
              <motion.span
                initial={{ scale: 0.7, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ duration: 0.15 }}
                style={{ originX: 0, originY: "50%" }}
                className={cn(
                  ["text-sm"],
                  [activeId === s.id ? "text-stone-600" : "text-stone-400"],
                )}
              >
                {s.label}
              </motion.span>
            ) : (
              <div className="h-px w-5 bg-stone-600" />
            )}
          </button>
        );
      })}
    </nav>
  );
}

function MobileTopBar({
  isMobileOpen,
  setIsMobileOpen,
}: {
  isMobileOpen: boolean;
  setIsMobileOpen: (open: boolean) => void;
}) {
  return (
    <div className="fixed top-0 right-0 left-0 z-50 flex h-14 items-center justify-between bg-neutral-950 px-4 md:hidden">
      <Link to="/">
        <CharLogo className="h-5 w-auto text-neutral-500" />
      </Link>
      <button
        onClick={() => setIsMobileOpen(!isMobileOpen)}
        className="text-fg-subtle hover:text-fg flex size-9 cursor-pointer items-center justify-center rounded-lg transition-colors"
        aria-label={isMobileOpen ? "Close menu" : "Open menu"}
      >
        {isMobileOpen ? <X size={20} /> : <Menu size={20} />}
      </button>
    </div>
  );
}

function SidebarFlyout({
  label,
  to,
  isActive,
  activeSubItem,
}: {
  label: string;
  to: string;
  isActive: boolean;
  activeSubItem: { to: string; label: string } | null;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const open = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    setIsOpen(true);
  };

  const close = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    setIsOpen(false);
  };

  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, []);

  return (
    <div className="relative" onMouseEnter={open} onMouseLeave={close}>
      <Link
        to={to}
        className={cn(
          [
            "flex items-center justify-between py-1 text-base transition-colors",
          ],
          [
            isActive
              ? "text-fg -mx-2 rounded-xl px-2 underline"
              : "text-fg hover:underline",
          ],
        )}
      >
        {label}
        <ChevronRight size={14} className="opacity-50" />
      </Link>

      {activeSubItem && (
        <Link
          to={activeSubItem.to}
          className="block pl-2 text-xs text-stone-500 transition-colors hover:text-stone-800"
        >
          {activeSubItem.label}
        </Link>
      )}

      <AnimatePresence>
        {isOpen && (
          <motion.div
            className="absolute top-0 left-full z-[9999] pl-2"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.1 }}
            onMouseEnter={open}
            onMouseLeave={close}
          >
            <div className="border-brand-color surface w-56 rounded-lg border py-2 shadow-lg">
              {label === "Product" && <ProductFlyoutContent />}
              {label === "Resources" && <ResourcesFlyoutContent />}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ProductFlyoutContent() {
  return (
    <div className="flex flex-col">
      <div className="px-3 pb-1">
        <span className="text-xs font-medium tracking-wide text-stone-400 uppercase">
          Features
        </span>
      </div>
      {featuresList.map((item) => (
        <Link
          key={item.to}
          to={item.to}
          className="px-3 py-1.5 text-sm text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950"
        >
          {item.label}
        </Link>
      ))}
      <div className="border-brand-color my-1.5 border-t" />
      <div className="px-3 pb-1">
        <span className="text-xs font-medium tracking-wide text-stone-400 uppercase">
          Solutions
        </span>
      </div>
      {solutionsList.map((item) => (
        <Link
          key={item.to}
          to={item.to}
          className="px-3 py-1.5 text-sm text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950"
        >
          {item.label}
        </Link>
      ))}
    </div>
  );
}

function ResourcesFlyoutContent() {
  return (
    <div className="flex flex-col">
      {resourcesList.map((item) => {
        const Icon = item.icon;
        if (item.external) {
          return (
            <a
              key={item.to}
              href={item.to}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2.5 px-3 py-1.5 text-sm text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950"
            >
              <Icon size={15} className="shrink-0 text-stone-400" />
              {item.label}
            </a>
          );
        }
        return (
          <Link
            key={item.to}
            to={item.to}
            className="flex items-center gap-2.5 px-3 py-1.5 text-sm text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950"
          >
            <Icon size={15} className="shrink-0 text-stone-400" />
            {item.label}
          </Link>
        );
      })}
    </div>
  );
}

function MobileSubmenu({
  label,
  isActive,
}: {
  label: string;
  to: string;
  isActive: boolean;
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <div>
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          [
            "flex w-full cursor-pointer items-center justify-between py-2.5 text-left text-base transition-colors",
          ],
          [
            isActive
              ? "-mx-2 rounded-lg px-2 text-neutral-200"
              : "text-neutral-500 hover:text-neutral-300",
          ],
        )}
      >
        {label}
        <ChevronRight
          size={14}
          className={cn([
            "transition-transform duration-200",
            isExpanded && "rotate-90",
          ])}
        />
      </button>

      {isExpanded && (
        <div className="flex flex-col pb-2 pl-3">
          {label === "Product" && (
            <>
              <span className="py-1 text-xs font-medium tracking-wide text-neutral-600 uppercase">
                Features
              </span>
              {featuresList.map((item) => (
                <Link
                  key={item.to}
                  to={item.to}
                  className="py-1.5 text-sm text-neutral-400 transition-colors hover:text-neutral-200"
                >
                  {item.label}
                </Link>
              ))}
              <span className="mt-2 py-1 text-xs font-medium tracking-wide text-neutral-600 uppercase">
                Solutions
              </span>
              {solutionsList.map((item) => (
                <Link
                  key={item.to}
                  to={item.to}
                  className="py-1.5 text-sm text-neutral-400 transition-colors hover:text-neutral-200"
                >
                  {item.label}
                </Link>
              ))}
            </>
          )}
          {label === "Resources" && (
            <>
              {resourcesList.map((item) => {
                const IconComp = item.icon;
                if (item.external) {
                  return (
                    <a
                      key={item.to}
                      href={item.to}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-2 py-1.5 text-sm text-neutral-400 transition-colors hover:text-neutral-200"
                    >
                      <IconComp
                        size={14}
                        className="shrink-0 text-neutral-600"
                      />
                      {item.label}
                    </a>
                  );
                }
                return (
                  <Link
                    key={item.to}
                    to={item.to}
                    className="flex items-center gap-2 py-1.5 text-sm text-neutral-400 transition-colors hover:text-neutral-200"
                  >
                    <IconComp size={14} className="shrink-0 text-neutral-600" />
                    {item.label}
                  </Link>
                );
              })}
            </>
          )}
        </div>
      )}
    </div>
  );
}

function SidebarCTA({
  platformCTA,
  visible = true,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
  visible?: boolean;
}) {
  const baseClass =
    "flex h-9 items-center justify-center rounded-lg bg-neutral-800 text-sm text-neutral-300 transition-colors hover:bg-neutral-700 hover:text-neutral-100";

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 8 }}
          transition={{ duration: 0.2 }}
        >
          {platformCTA.action === "download" ? (
            <a href="/download/apple-silicon" download className={baseClass}>
              {platformCTA.label}
            </a>
          ) : (
            <Link to="/" className={baseClass}>
              {platformCTA.label}
            </Link>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
