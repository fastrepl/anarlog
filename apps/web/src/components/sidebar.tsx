import { Link, useRouterState } from "@tanstack/react-router";
import {
  BookOpen,
  Building2,
  ChevronDown,
  ChevronRight,
  ChevronUp,
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
import { AnimatePresence, motion } from "motion/react";
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
  { id: "opensource", label: "Open source" },
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
  const [isProductOpen, setIsProductOpen] = useState(false);
  const [isResourcesOpen, setIsResourcesOpen] = useState(false);
  const router = useRouterState();
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);
  const pathname = router.location.pathname;
  const isHomePage = pathname === "/";
  const activeSection = useActiveHomeSection(isHomePage);
  const activeSubItem = isHomePage ? null : findActiveSubItem(pathname);

  useEffect(() => {
    setIsMobileOpen(false);
    setIsProductOpen(false);
    setIsResourcesOpen(false);
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
      {/* ===== MOBILE: top bar + dropdown menu (<md / <768px) ===== */}
      <div className="fixed top-0 right-0 left-0 z-50 flex h-14 items-center justify-between border-b border-neutral-100 bg-white/80 px-4 backdrop-blur-xs md:hidden">
        <Link to="/">
          <CharLogo className="text-fg h-5 w-auto" />
        </Link>
        <div className="flex items-center gap-3">
          <CTAButton platformCTA={platformCTA} mobile />
          <button
            onClick={() => setIsMobileOpen(!isMobileOpen)}
            className="flex h-8 cursor-pointer items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-3 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
            aria-label={isMobileOpen ? "Close menu" : "Open menu"}
          >
            {isMobileOpen ? (
              <X className="text-neutral-600" size={16} />
            ) : (
              <Menu className="text-neutral-600" size={16} />
            )}
          </button>
        </div>
      </div>

      {/* Mobile dropdown menu */}
      {isMobileOpen && (
        <>
          <div
            className="fixed inset-0 z-40 md:hidden"
            onClick={() => setIsMobileOpen(false)}
          />
          <div className="animate-in slide-in-from-top fixed top-14 right-0 left-0 z-50 max-h-[calc(100vh-56px)] overflow-y-auto border-b border-neutral-100 bg-white/80 shadow-lg backdrop-blur-xs duration-300 md:hidden">
            <nav className="mx-auto max-w-6xl px-4 py-6">
              <div className="flex flex-col gap-6">
                <MobileMenuLinks
                  isProductOpen={isProductOpen}
                  setIsProductOpen={setIsProductOpen}
                  isResourcesOpen={isResourcesOpen}
                  setIsResourcesOpen={setIsResourcesOpen}
                  setIsMenuOpen={setIsMobileOpen}
                />
                <MobileMenuCTAs
                  platformCTA={platformCTA}
                  setIsMenuOpen={setIsMobileOpen}
                />
              </div>
            </nav>
          </div>
        </>
      )}

      {/* ===== TABLET: horizontal header bar (md to xl / 768-1280px) ===== */}
      <header className="fixed top-0 right-0 left-0 z-50 hidden border-b border-neutral-100 bg-white/80 backdrop-blur-xs md:block xl:hidden">
        <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-4">
          <div className="flex items-center gap-6">
            <Link to="/" className="mr-2">
              <CharLogo className="text-fg h-6 w-auto" />
            </Link>
            <Link
              to="/why-char/"
              className="text-sm text-neutral-600 decoration-dotted transition-colors hover:text-neutral-800 hover:underline"
            >
              Why Char
            </Link>
            <TabletDropdown
              label="Product"
              isOpen={isProductOpen}
              setIsOpen={setIsProductOpen}
            >
              <div className="grid grid-cols-2 gap-x-6 px-3 py-2">
                <div>
                  <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                    Features
                  </div>
                  {featuresList.map((item) => (
                    <Link
                      key={item.to}
                      to={item.to}
                      onClick={() => setIsProductOpen(false)}
                      className="group flex items-center py-2 text-sm text-neutral-700"
                    >
                      <span className="decoration-dotted group-hover:underline">
                        {item.label}
                      </span>
                    </Link>
                  ))}
                </div>
                <div>
                  <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                    Solutions
                  </div>
                  {solutionsList.map((item) => (
                    <Link
                      key={item.to}
                      to={item.to}
                      onClick={() => setIsProductOpen(false)}
                      className="group flex items-center py-2 text-sm text-neutral-700"
                    >
                      <span className="decoration-dotted group-hover:underline">
                        {item.label}
                      </span>
                    </Link>
                  ))}
                </div>
              </div>
            </TabletDropdown>
            <TabletDropdown
              label="Resources"
              isOpen={isResourcesOpen}
              setIsOpen={setIsResourcesOpen}
            >
              <div className="px-3 py-2">
                {resourcesList.map((item) => {
                  const Icon = item.icon;
                  if (item.external) {
                    return (
                      <a
                        key={item.to}
                        href={item.to}
                        target="_blank"
                        rel="noopener noreferrer"
                        onClick={() => setIsResourcesOpen(false)}
                        className="group flex items-center gap-2 py-2 text-sm text-neutral-700"
                      >
                        <Icon size={16} className="text-neutral-400" />
                        <span className="decoration-dotted group-hover:underline">
                          {item.label}
                        </span>
                      </a>
                    );
                  }
                  return (
                    <Link
                      key={item.to}
                      to={item.to}
                      onClick={() => setIsResourcesOpen(false)}
                      className="group flex items-center gap-2 py-2 text-sm text-neutral-700"
                    >
                      <Icon size={16} className="text-neutral-400" />
                      <span className="decoration-dotted group-hover:underline">
                        {item.label}
                      </span>
                    </Link>
                  );
                })}
              </div>
            </TabletDropdown>
            <Link
              to="/pricing/"
              className="text-sm text-neutral-600 decoration-dotted transition-colors hover:text-neutral-800 hover:underline"
            >
              Pricing
            </Link>
          </div>
          <nav className="flex items-center gap-4">
            <SearchTrigger variant="header" />
            <CTAButton platformCTA={platformCTA} />
          </nav>
        </div>
      </header>

      {/* ===== DESKTOP: left sidebar (xl+ / 1280px+) ===== */}
      <aside className="wide:w-[200px] z-10 hidden w-[120px] shrink-0 self-stretch xl:block">
        <div className="sticky top-0 flex h-screen flex-col">
          <div className="wide:px-12 px-6 pt-12 pb-10">
            <Link to="/">
              <CharLogo className="text-fg wide:h-8 h-6 w-auto transition-colors hover:scale-105" />
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
                <div className="wide:px-12 px-6 pb-4">
                  <HomeSectionNav activeId={activeSection} />
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          <nav className="wide:px-12 flex flex-col gap-1 px-6 pt-4">
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

          <div className="wide:px-12 shrink-0 px-6 pb-8">
            <div className="flex flex-col gap-3">
              <SearchTrigger variant="header" />
            </div>
          </div>
        </div>
      </aside>
    </>
  );
}

// ─── Tablet dropdown (md–xl) ────────────────────────────────────────────────

function TabletDropdown({
  label,
  isOpen,
  setIsOpen,
  children,
}: {
  label: string;
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className="relative"
      onMouseEnter={() => setIsOpen(true)}
      onMouseLeave={() => setIsOpen(false)}
    >
      <button className="flex items-center gap-1 py-2 text-sm text-neutral-600 transition-all hover:text-neutral-800">
        {label}
        {isOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isOpen && (
        <div className="absolute top-full left-0 z-50 w-max min-w-56 pt-2">
          <div className="rounded-xs border border-neutral-200 bg-white py-2 shadow-lg">
            {children}
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Mobile menu (<md) ──────────────────────────────────────────────────────

function MobileMenuLinks({
  isProductOpen,
  setIsProductOpen,
  isResourcesOpen,
  setIsResourcesOpen,
  setIsMenuOpen,
}: {
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <Link
        to="/why-char/"
        onClick={() => setIsMenuOpen(false)}
        className="block text-base text-neutral-700 transition-colors hover:text-neutral-900"
      >
        Why Char
      </Link>
      <MobileProductSection
        isProductOpen={isProductOpen}
        setIsProductOpen={setIsProductOpen}
        setIsMenuOpen={setIsMenuOpen}
      />
      <MobileResourcesSection
        isResourcesOpen={isResourcesOpen}
        setIsResourcesOpen={setIsResourcesOpen}
        setIsMenuOpen={setIsMenuOpen}
      />
      <Link
        to="/pricing/"
        onClick={() => setIsMenuOpen(false)}
        className="block text-base text-neutral-700 transition-colors hover:text-neutral-900"
      >
        Pricing
      </Link>
    </div>
  );
}

function MobileProductSection({
  isProductOpen,
  setIsProductOpen,
  setIsMenuOpen,
}: {
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div>
      <button
        onClick={() => setIsProductOpen(!isProductOpen)}
        className="flex w-full items-center justify-between text-base text-neutral-700 transition-colors hover:text-neutral-900"
      >
        <span>Product</span>
        {isProductOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isProductOpen && (
        <div className="mt-3 ml-4 flex flex-col gap-4 border-l-2 border-neutral-200 pl-4">
          <div>
            <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Features
            </div>
            <div className="flex flex-col gap-2 pb-4">
              {featuresList.map((item) => (
                <Link
                  key={item.to}
                  to={item.to}
                  onClick={() => setIsMenuOpen(false)}
                  className="py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
                >
                  {item.label}
                </Link>
              ))}
            </div>
          </div>
          <div>
            <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Solutions
            </div>
            <div className="flex flex-col gap-2">
              {solutionsList.map((item) => (
                <Link
                  key={item.to}
                  to={item.to}
                  onClick={() => setIsMenuOpen(false)}
                  className="py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
                >
                  {item.label}
                </Link>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function MobileResourcesSection({
  isResourcesOpen,
  setIsResourcesOpen,
  setIsMenuOpen,
}: {
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div>
      <button
        onClick={() => setIsResourcesOpen(!isResourcesOpen)}
        className="flex w-full items-center justify-between text-base text-neutral-700 transition-colors hover:text-neutral-900"
      >
        <span>Resources</span>
        {isResourcesOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isResourcesOpen && (
        <div className="mt-3 ml-4 flex flex-col gap-2 border-l-2 border-neutral-200 pl-4">
          {resourcesList.map((item) => {
            const Icon = item.icon;
            if (item.external) {
              return (
                <a
                  key={item.to}
                  href={item.to}
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={() => setIsMenuOpen(false)}
                  className="flex items-center gap-2 py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
                >
                  <Icon size={14} className="text-neutral-400" />
                  {item.label}
                </a>
              );
            }
            return (
              <Link
                key={item.to}
                to={item.to}
                onClick={() => setIsMenuOpen(false)}
                className="flex items-center gap-2 py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
              >
                <Icon size={14} className="text-neutral-400" />
                {item.label}
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}

function MobileMenuCTAs({
  platformCTA,
  setIsMenuOpen,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div className="flex flex-row gap-3">
      <Link
        to="/auth/"
        search={{ flow: "web" }}
        onClick={() => setIsMenuOpen(false)}
        className="block w-full rounded-lg border border-neutral-200 bg-white px-4 py-3 text-center text-sm text-neutral-700 transition-colors hover:bg-neutral-50"
      >
        Get started
      </Link>
      {platformCTA.action === "download" ? (
        <a
          href="/download/apple-silicon"
          download
          onClick={() => setIsMenuOpen(false)}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          {platformCTA.label}
        </a>
      ) : (
        <Link
          to="/"
          onClick={() => setIsMenuOpen(false)}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          {platformCTA.label}
        </Link>
      )}
    </div>
  );
}

// ─── Desktop sidebar pieces (xl+) ──────────────────────────────────────────

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
                  [activeId === s.id ? "text-fg" : "text-fg-subtle"],
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

// ─── Shared CTA button ──────────────────────────────────────────────────────

function CTAButton({
  platformCTA,
  mobile = false,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
  mobile?: boolean;
}) {
  const baseClass = mobile
    ? "px-4 h-8 flex items-center text-sm bg-linear-to-t from-stone-600 to-stone-500 text-white rounded-full shadow-md active:scale-[98%] transition-all"
    : "px-4 h-8 flex items-center text-sm bg-linear-to-t from-stone-600 to-stone-500 text-white rounded-full shadow-md hover:shadow-lg hover:scale-[102%] active:scale-[98%] transition-all";

  if (platformCTA.action === "download") {
    return (
      <a href="/download/apple-silicon" download className={baseClass}>
        {platformCTA.label}
      </a>
    );
  }

  return (
    <Link to="/" className={baseClass}>
      {platformCTA.label}
    </Link>
  );
}
