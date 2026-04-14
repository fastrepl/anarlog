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
  Menu,
  MessageCircle,
  Newspaper,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import { cn } from "@hypr/utils";

import { SearchTrigger } from "@/components/search";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";
import {
  allSolutionMenuItems,
  allSolutionsMenuItem,
  featuredSolutionMenuItems,
  showMoreSolutionsMenuItem,
} from "@/lib/solutions";

type MenuItem = {
  to: string;
  label: string;
  icon?: LucideIcon;
  external?: boolean;
};

type MenuGroup = {
  title: string;
  items: MenuItem[];
};

const featuresList: MenuItem[] = [
  { to: "/product/ai-notetaking", label: "AI Notetaking" },
  { to: "/product/search", label: "Searchable Notes" },
  { to: "/gallery?type=template", label: "Custom Templates" },
  { to: "/product/markdown", label: "Markdown Files" },
  { to: "/product/flexible-ai", label: "Flexible AI" },
  { to: "/opensource", label: "Open Source" },
];

const solutionsList: MenuItem[] = featuredSolutionMenuItems;

const resourcesList: MenuItem[] = [
  { to: "/blog/", label: "Blog", icon: FileText },
  { to: "/docs/", label: "Documentation", icon: BookOpen },
  {
    to: "/gallery?type=template",
    label: "Meeting Templates",
    icon: LayoutTemplate,
  },
  { to: "/updates/", label: "Updates", icon: Newspaper },
  { to: "/changelog/", label: "Changelog", icon: History },
  { to: "/company-handbook/", label: "Company Handbook", icon: Building2 },
  {
    to: "https://discord.gg/hyprnote",
    label: "Community",
    icon: MessageCircle,
    external: true,
  },
];

const productGroups: MenuGroup[] = [
  { title: "Features", items: featuresList },
  { title: "Solutions", items: solutionsList },
];

const navLinks = [
  { to: "/why-char/", label: "Why Char" },
  {
    to: "/product/ai-notetaking/",
    label: "Product",
    submenu: "product" as const,
  },
  { to: "/docs/", label: "Resources", submenu: "resources" as const },
  { to: "/pricing/", label: "Pricing" },
] as const;

const MAIN_MENU_LINK_HOVER =
  "hover:underline hover:decoration-dotted hover:underline-offset-4";

function isPathActive(pathname: string, to: string) {
  return pathname.startsWith(to.replace(/\/$/, ""));
}

function findActiveSubItem(pathname: string) {
  const candidates = [
    ...featuresList.map((i) => ({ ...i, parent: "Product" })),
    ...allSolutionMenuItems.map((i) => ({ ...i, parent: "Product" })),
    { ...allSolutionsMenuItem, parent: "Product" as const },
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

export function CharLogo({
  className,
  compact,
}: {
  className?: string;
  compact?: boolean;
}) {
  if (compact) {
    return (
      <svg
        width="30"
        height="30"
        viewBox="0 0 30 30"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className={className}
      >
        <path
          d="M7.871 4.147C7.871 5.658 7.082 7.039 6.099 8.214C4.65 9.946 3.77 12.161 3.77 14.575C3.77 16.99 4.65 19.205 6.099 20.937C7.082 22.112 7.871 23.493 7.871 25.004V29.151H2.965V24.319C2.965 22.735 2.165 21.249 0.822 20.34L0 19.783V9.235L0.822 8.678C2.165 7.769 2.965 6.284 2.965 4.699V0L7.871 0V4.147Z"
          fill="currentColor"
        />
        <g transform="translate(-76, 0)">
          <path
            d="M94.746 4.147C94.746 5.658 95.535 7.039 96.519 8.214C97.967 9.946 98.847 12.161 98.847 14.575C98.847 16.99 97.967 19.205 96.519 20.937C95.535 22.112 94.746 23.493 94.746 25.004V29.151H99.653V24.319C99.653 22.735 100.452 21.249 101.795 20.34L102.617 19.783V9.235L101.795 8.678C100.452 7.769 99.653 6.284 99.653 4.699V0L94.746 0V4.147Z"
            fill="currentColor"
          />
        </g>
      </svg>
    );
  }

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

export function Sidebar() {
  const [isMobileOpen, setIsMobileOpen] = useState(false);
  const [isProductOpen, setIsProductOpen] = useState(false);
  const [isResourcesOpen, setIsResourcesOpen] = useState(false);
  const router = useRouterState();
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);
  const pathname = router.location.pathname;
  const activeSubItem = findActiveSubItem(pathname);
  const closeAllMenus = () => {
    setIsMobileOpen(false);
    setIsProductOpen(false);
    setIsResourcesOpen(false);
  };

  useEffect(() => {
    closeAllMenus();
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
      <div className="fixed top-[var(--announcement-bar-h,0px)] right-0 left-0 z-50 flex h-14 items-center justify-between border-b border-neutral-100 bg-white/80 px-4 backdrop-blur-xs md:hidden">
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
          <div className="animate-in slide-in-from-top fixed top-[calc(theme(spacing.14)+var(--announcement-bar-h,0px))] right-0 left-0 z-50 max-h-[calc(100vh-56px-var(--announcement-bar-h,0px))] overflow-y-auto border-b border-neutral-100 bg-white/80 shadow-lg backdrop-blur-xs duration-300 md:hidden">
            <nav className="mx-auto max-w-6xl px-4 py-6">
              <div className="flex flex-col gap-6">
                <MobileMenuLinks
                  isProductOpen={isProductOpen}
                  setIsProductOpen={setIsProductOpen}
                  isResourcesOpen={isResourcesOpen}
                  setIsResourcesOpen={setIsResourcesOpen}
                  closeAllMenus={closeAllMenus}
                />
                <MobileMenuCTAs
                  platformCTA={platformCTA}
                  closeAllMenus={closeAllMenus}
                />
              </div>
            </nav>
          </div>
        </>
      )}

      {/* ===== TABLET: horizontal header bar (md to xl / 768-1280px) ===== */}
      <header className="fixed top-[var(--announcement-bar-h,0px)] right-0 left-0 z-50 hidden border-b border-neutral-100 bg-white/80 backdrop-blur-xs md:block xl:hidden">
        <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-4">
          <div className="flex items-center gap-6">
            <Link to="/" className="mr-2">
              <CharLogo className="text-fg hidden h-6 w-auto lg:block" />
              <CharLogo compact className="text-fg h-6 w-auto lg:hidden" />
            </Link>
            <Link
              to="/why-char/"
              className={cn(
                [
                  "text-sm text-neutral-600 decoration-dotted transition-colors hover:text-neutral-800",
                ],
                [MAIN_MENU_LINK_HOVER],
              )}
            >
              Why Char
            </Link>
            <TabletDropdown
              label="Product"
              isOpen={isProductOpen}
              setIsOpen={setIsProductOpen}
            >
              <ProductMenuContent
                variant="tablet"
                onItemClick={() => setIsProductOpen(false)}
              />
            </TabletDropdown>
            <TabletDropdown
              label="Resources"
              isOpen={isResourcesOpen}
              setIsOpen={setIsResourcesOpen}
            >
              <ResourcesMenuContent
                variant="tablet"
                onItemClick={() => setIsResourcesOpen(false)}
              />
            </TabletDropdown>
            <Link
              to="/pricing/"
              className={cn(
                [
                  "text-sm text-neutral-600 decoration-dotted transition-colors hover:text-neutral-800",
                ],
                [MAIN_MENU_LINK_HOVER],
              )}
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
      <aside className="wide:w-[160px] z-50 hidden w-[120px] shrink-0 self-stretch xl:block">
        <div className="sticky top-0 flex flex-col">
          <div className="wide:px-4 px-4 pt-12 pb-4">
            <Link to="/">
              <CharLogo className="text-fg wide:h-8 h-6 w-auto transition-colors hover:scale-105" />
            </Link>
          </div>

          <nav className="wide:px-4 flex flex-col gap-1 pt-4 pl-4">
            {navLinks.map((link) =>
              "submenu" in link ? (
                <SidebarFlyout
                  key={link.to}
                  label={link.label}
                  to={link.to}
                  submenu={link.submenu}
                  isActive={isPathActive(pathname, link.to)}
                  activeSubItem={
                    activeSubItem?.parent === link.label ? activeSubItem : null
                  }
                />
              ) : (
                <Link
                  key={link.to}
                  to={link.to}
                  className={cn(
                    ["py-1 text-base transition-colors"],
                    [
                      isPathActive(pathname, link.to)
                        ? "text-fg -mx-2 rounded-full px-2 underline decoration-dotted underline-offset-2"
                        : cn(["text-fg"], [MAIN_MENU_LINK_HOVER]),
                    ],
                  )}
                >
                  {link.label}
                </Link>
              ),
            )}
            <SearchTrigger variant="sidebar-nav" />
          </nav>

          <div className="flex-1" />
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
      <button
        className={cn(
          [
            "flex items-center gap-1 py-2 font-sans text-sm text-neutral-600 decoration-dotted transition-colors hover:text-neutral-800",
          ],
          [MAIN_MENU_LINK_HOVER],
        )}
      >
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
  closeAllMenus,
}: {
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
  closeAllMenus: () => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <Link
        to="/why-char/"
        onClick={closeAllMenus}
        className={cn(
          [
            "block text-base text-neutral-700 decoration-dotted transition-colors hover:text-neutral-900",
          ],
          [MAIN_MENU_LINK_HOVER],
        )}
      >
        Why Char
      </Link>
      <CollapsibleSection
        label="Product"
        isOpen={isProductOpen}
        setIsOpen={setIsProductOpen}
      >
        <ProductMenuContent variant="mobile" onItemClick={closeAllMenus} />
      </CollapsibleSection>
      <CollapsibleSection
        label="Resources"
        isOpen={isResourcesOpen}
        setIsOpen={setIsResourcesOpen}
      >
        <ResourcesMenuContent variant="mobile" onItemClick={closeAllMenus} />
      </CollapsibleSection>
      <Link
        to="/pricing/"
        onClick={closeAllMenus}
        className={cn(
          [
            "block text-base text-neutral-700 decoration-dotted transition-colors hover:text-neutral-900",
          ],
          [MAIN_MENU_LINK_HOVER],
        )}
      >
        Pricing
      </Link>
    </div>
  );
}

function MobileMenuCTAs({
  platformCTA,
  closeAllMenus,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
  closeAllMenus: () => void;
}) {
  return (
    <div className="flex flex-row gap-3">
      <Link
        to="/auth/"
        search={{ flow: "web" }}
        onClick={closeAllMenus}
        className="block w-full rounded-lg border border-neutral-200 bg-white px-4 py-3 text-center text-sm text-neutral-700 transition-colors hover:bg-neutral-50"
      >
        Get started
      </Link>
      {platformCTA.action === "download" ? (
        <a
          href="/download/apple-silicon"
          download
          onClick={closeAllMenus}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          {platformCTA.label}
        </a>
      ) : (
        <Link
          to="/"
          onClick={closeAllMenus}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          {platformCTA.label}
        </Link>
      )}
    </div>
  );
}

// ─── Desktop sidebar pieces (xl+) ──────────────────────────────────────────

function MenuItemLink({
  item,
  onClick,
  className,
  iconClassName,
  iconSize = 14,
  decorateOnHover = false,
}: {
  item: MenuItem;
  onClick?: () => void;
  className: string;
  iconClassName?: string;
  iconSize?: number;
  decorateOnHover?: boolean;
}) {
  const Icon = item.icon;
  const label = decorateOnHover ? (
    <span className="decoration-dotted group-hover:underline">
      {item.label}
    </span>
  ) : (
    item.label
  );

  const content = (
    <>
      {Icon ? <Icon size={iconSize} className={iconClassName} /> : null}
      {label}
    </>
  );

  if (item.external) {
    return (
      <a
        href={item.to}
        target="_blank"
        rel="noopener noreferrer"
        onClick={onClick}
        className={className}
      >
        {content}
      </a>
    );
  }

  return (
    <Link to={item.to} onClick={onClick} className={className}>
      {content}
    </Link>
  );
}

function MenuGroupLinks({
  group,
  titleClassName,
  listClassName,
  itemClassName,
  onItemClick,
  decorateOnHover = false,
  footer,
}: {
  group: MenuGroup;
  titleClassName: string;
  listClassName: string;
  itemClassName: string;
  onItemClick?: () => void;
  decorateOnHover?: boolean;
  footer?: React.ReactNode;
}) {
  return (
    <div>
      <div className={titleClassName}>{group.title}</div>
      <div className={listClassName}>
        {group.items.map((item) => (
          <MenuItemLink
            key={item.to}
            item={item}
            onClick={onItemClick}
            className={itemClassName}
            decorateOnHover={decorateOnHover}
          />
        ))}
      </div>
      {footer}
    </div>
  );
}

function SolutionsIndexLink({
  className,
  onClick,
  decorateOnHover = false,
}: {
  className: string;
  onClick?: () => void;
  decorateOnHover?: boolean;
}) {
  const label = decorateOnHover ? (
    <span className="decoration-dotted group-hover:underline">
      {showMoreSolutionsMenuItem.label}
    </span>
  ) : (
    showMoreSolutionsMenuItem.label
  );

  return (
    <Link
      to={showMoreSolutionsMenuItem.to}
      onClick={onClick}
      className={className}
    >
      {label}
    </Link>
  );
}

function ProductMenuContent({
  variant,
  onItemClick,
}: {
  variant: "mobile" | "tablet" | "flyout";
  onItemClick?: () => void;
}) {
  if (variant === "tablet") {
    return (
      <div className="grid grid-cols-2 gap-x-4 px-2 py-2">
        {productGroups.map((group) => (
          <MenuGroupLinks
            key={group.title}
            group={group}
            titleClassName="mb-2 text-xs font-semibold tracking-wider text-fg-subtle uppercase"
            listClassName="flex flex-col"
            itemClassName="group flex items-center py-2 text-sm text-fg"
            onItemClick={onItemClick}
            decorateOnHover
            footer={
              group.title === "Solutions" ? (
                <SolutionsIndexLink
                  className="group text-fg mt-1 inline-flex py-2 text-sm font-medium"
                  onClick={onItemClick}
                  decorateOnHover
                />
              ) : null
            }
          />
        ))}
      </div>
    );
  }

  if (variant === "mobile") {
    return (
      <div className="mt-3 ml-4 flex flex-col gap-4 border-l-2 border-neutral-200 pl-4">
        {productGroups.map((group, index) => (
          <MenuGroupLinks
            key={group.title}
            group={group}
            titleClassName="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase"
            listClassName={
              index === 0 ? "flex flex-col gap-2 pb-4" : "flex flex-col gap-2"
            }
            itemClassName="py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
            onItemClick={onItemClick}
            footer={
              group.title === "Solutions" ? (
                <SolutionsIndexLink
                  className="mt-2 inline-flex py-1 text-sm font-medium text-neutral-700 transition-colors hover:text-neutral-900"
                  onClick={onItemClick}
                />
              ) : null
            }
          />
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {productGroups.map((group, index) => (
        <div key={group.title}>
          {index > 0 ? (
            <div className="border-color-brand my-1.5 border-t" />
          ) : null}
          <MenuGroupLinks
            group={group}
            titleClassName="px-3 pb-1 text-xs font-medium tracking-wide text-fg-subtle uppercase"
            listClassName="flex flex-col"
            itemClassName="px-3 py-1.5 text-sm text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950"
            footer={
              group.title === "Solutions" ? (
                <SolutionsIndexLink className="mt-1 inline-flex px-3 py-1.5 text-sm font-medium text-stone-700 transition-colors hover:bg-stone-50 hover:text-stone-950" />
              ) : null
            }
          />
        </div>
      ))}
    </div>
  );
}

function ResourcesMenuContent({
  variant,
  onItemClick,
}: {
  variant: "mobile" | "tablet" | "flyout";
  onItemClick?: () => void;
}) {
  if (variant === "tablet") {
    return (
      <div className="px-3 py-2">
        {resourcesList.map((item) => (
          <MenuItemLink
            key={item.to}
            item={item}
            onClick={onItemClick}
            className="group flex items-center gap-2 py-2 text-sm text-neutral-700"
            iconClassName="text-neutral-400"
            iconSize={16}
            decorateOnHover
          />
        ))}
      </div>
    );
  }

  if (variant === "mobile") {
    return (
      <div className="mt-3 ml-4 flex flex-col gap-2 border-l-2 border-neutral-200 pl-4">
        {resourcesList.map((item) => (
          <MenuItemLink
            key={item.to}
            item={item}
            onClick={onItemClick}
            className="text-fg flex items-center gap-2 py-1 text-sm transition-colors hover:text-neutral-900"
            iconClassName="text-neutral-400"
            iconSize={14}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {resourcesList.map((item) => (
        <MenuItemLink
          key={item.to}
          item={item}
          className="text-fg flex items-center gap-2.5 px-3 py-1.5 text-sm transition-colors hover:bg-stone-50 hover:text-stone-950"
          iconClassName="shrink-0 text-fg-subtle"
          iconSize={15}
        />
      ))}
    </div>
  );
}

function CollapsibleSection({
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
    <div>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          [
            "flex w-full items-center justify-between text-base text-neutral-700 decoration-dotted transition-colors hover:text-neutral-900",
          ],
          [MAIN_MENU_LINK_HOVER],
        )}
      >
        <span>{label}</span>
        {isOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isOpen ? children : null}
    </div>
  );
}

function SidebarFlyout({
  label,
  to,
  submenu,
  isActive,
  activeSubItem,
}: {
  label: string;
  to: string;
  submenu: "product" | "resources";
  isActive: boolean;
  activeSubItem: { to: string; label: string } | null;
}) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div
      className="relative"
      onMouseEnter={() => setIsOpen(true)}
      onMouseLeave={() => setIsOpen(false)}
    >
      <Link
        to={to}
        className={cn(
          [
            "transition-position flex items-center justify-between py-1 text-base",
          ],
          [
            isActive
              ? "text-fg -mx-2 rounded-xl px-2 underline"
              : cn(["text-fg"], [MAIN_MENU_LINK_HOVER]),
          ],
        )}
      >
        {label}
        <ChevronRight size={14} className="opacity-50" />
      </Link>

      {activeSubItem && (
        <Link
          to={activeSubItem.to}
          className="text-fg block text-xs opacity-50 transition-opacity hover:opacity-100"
        >
          {activeSubItem.label}
        </Link>
      )}

      <AnimatePresence>
        {isOpen && (
          <motion.div
            className="absolute top-0 left-full z-[9999] pl-2"
            initial={{ opacity: 0, x: 8 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 8 }}
            transition={{ duration: 0.15, ease: "easeInOut" }}
          >
            <div className="border-color-brand surface w-56 rounded-lg border py-2 shadow-lg">
              {submenu === "product" ? (
                <ProductMenuContent variant="flyout" />
              ) : (
                <ResourcesMenuContent variant="flyout" />
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
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
