import { Link, useRouterState } from "@tanstack/react-router";
import {
  BookOpen,
  Building2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  FileText,
  History,
  LayoutTemplate,
  Menu,
  MessageCircle,
  Newspaper,
  PanelLeft,
  PanelLeftClose,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { SearchTrigger } from "@/components/search";
import { useBlogToc } from "@/hooks/use-blog-toc";
import { useDocsDrawer } from "@/hooks/use-docs-drawer";
import { useHandbookDrawer } from "@/hooks/use-handbook-drawer";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

function scrollToHero() {
  const heroElement = document.getElementById("hero");
  if (heroElement) {
    heroElement.scrollIntoView({ behavior: "smooth", block: "start" });
  }
}

function getMaxWidthClass(pathname: string): string {
  const isBlogOrDocs =
    pathname.startsWith("/blog") || pathname.startsWith("/docs");
  return isBlogOrDocs ? "max-w-6xl" : "max-w-6xl";
}

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
  { to: "/company-handbook/", label: "Company Handbook", icon: Building2 },
  {
    to: "https://discord.gg/hyprnote",
    label: "Community",
    icon: MessageCircle,
    external: true,
  },
];

export function Header() {
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [isProductOpen, setIsProductOpen] = useState(false);
  const [isResourcesOpen, setIsResourcesOpen] = useState(false);
  const [showMobileHeader, setShowMobileHeader] = useState(true);
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);
  const router = useRouterState();
  const maxWidthClass = getMaxWidthClass(router.location.pathname);
  const isDocsPage = router.location.pathname.startsWith("/docs");
  const isHandbookPage =
    router.location.pathname.startsWith("/company-handbook");
  const isBlogArticlePage =
    router.location.pathname.startsWith("/blog/") &&
    router.location.pathname !== "/blog/";
  const docsDrawer = useDocsDrawer();
  const handbookDrawer = useHandbookDrawer();
  const blogToc = useBlogToc();
  const lastScrollY = useRef(0);

  useEffect(() => {
    if (!isDocsPage && !isHandbookPage) {
      return;
    }

    const handleScroll = () => {
      if (window.innerWidth >= 768) {
        return;
      }

      const currentScrollY = window.scrollY;

      if (currentScrollY < 10) {
        setShowMobileHeader(true);
      } else if (currentScrollY > lastScrollY.current) {
        setShowMobileHeader(false);
      } else if (currentScrollY < lastScrollY.current) {
        setShowMobileHeader(true);
      }

      lastScrollY.current = currentScrollY;
    };

    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, [isDocsPage, isHandbookPage]);

  return (
    <>
      <header
        className={`fixed top-0 right-0 left-0 z-50 border-b border-neutral-100 bg-white/80 backdrop-blur-xs max-md:transition-transform max-md:duration-300 ${
          showMobileHeader ? "max-md:translate-y-0" : "max-md:-translate-y-full"
        }`}
      >
        <div
          className={`${maxWidthClass} laptop:px-0 mx-auto h-17.25 border-x border-neutral-100 px-4`}
        >
          <div className="flex h-full items-center justify-between">
            <LeftNav
              isDocsPage={isDocsPage}
              isHandbookPage={isHandbookPage}
              docsDrawer={docsDrawer}
              handbookDrawer={handbookDrawer}
              setIsMenuOpen={setIsMenuOpen}
              isProductOpen={isProductOpen}
              setIsProductOpen={setIsProductOpen}
              isResourcesOpen={isResourcesOpen}
              setIsResourcesOpen={setIsResourcesOpen}
            />
            <DesktopNav platformCTA={platformCTA} />
            <MobileNav
              platform={platform}
              platformCTA={platformCTA}
              isMenuOpen={isMenuOpen}
              setIsMenuOpen={setIsMenuOpen}
              docsDrawer={docsDrawer}
              handbookDrawer={handbookDrawer}
              isDocsPage={isDocsPage}
              isHandbookPage={isHandbookPage}
            />
          </div>
        </div>
        {(isDocsPage || isHandbookPage) && (
          <div
            className={`${maxWidthClass} mx-auto border-x border-neutral-100 px-4 py-2 md:hidden`}
          >
            <SearchTrigger variant="mobile" />
          </div>
        )}
        {isBlogArticlePage && blogToc && blogToc.toc.length > 0 && (
          <BlogTocSubBar blogToc={blogToc} maxWidthClass={maxWidthClass} />
        )}
      </header>

      {/* Spacer to account for fixed header */}
      <div
        className={
          isDocsPage || isHandbookPage
            ? "h-17.25 max-md:h-[calc(69px+52px)] md:h-17.25"
            : isBlogArticlePage && blogToc && blogToc.toc.length > 0
              ? "h-[calc(69px+44px)] sm:h-17.25"
              : "h-17.25"
        }
      />

      <MobileMenu
        isMenuOpen={isMenuOpen}
        setIsMenuOpen={setIsMenuOpen}
        isProductOpen={isProductOpen}
        setIsProductOpen={setIsProductOpen}
        isResourcesOpen={isResourcesOpen}
        setIsResourcesOpen={setIsResourcesOpen}
        platform={platform}
        platformCTA={platformCTA}
        maxWidthClass={maxWidthClass}
      />
    </>
  );
}

function LeftNav({
  isDocsPage,
  isHandbookPage,
  docsDrawer,
  handbookDrawer,
  setIsMenuOpen,
  isProductOpen,
  setIsProductOpen,
  isResourcesOpen,
  setIsResourcesOpen,
}: {
  isDocsPage: boolean;
  isHandbookPage: boolean;
  docsDrawer: ReturnType<typeof useDocsDrawer>;
  handbookDrawer: ReturnType<typeof useHandbookDrawer>;
  setIsMenuOpen: (open: boolean) => void;
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-4">
      <DrawerButton
        isDocsPage={isDocsPage}
        isHandbookPage={isHandbookPage}
        docsDrawer={docsDrawer}
        handbookDrawer={handbookDrawer}
        setIsMenuOpen={setIsMenuOpen}
      />
      <Logo />
      <Link
        to="/why-char/"
        className="hidden text-sm text-neutral-600 decoration-dotted transition-all hover:text-neutral-800 hover:underline md:block"
      >
        Why Char
      </Link>
      <ProductDropdown
        isProductOpen={isProductOpen}
        setIsProductOpen={setIsProductOpen}
      />
      <ResourcesDropdown
        isResourcesOpen={isResourcesOpen}
        setIsResourcesOpen={setIsResourcesOpen}
      />
      <NavLinks />
    </div>
  );
}

function DrawerButton({
  isDocsPage,
  isHandbookPage,
  docsDrawer,
  handbookDrawer,
  setIsMenuOpen,
}: {
  isDocsPage: boolean;
  isHandbookPage: boolean;
  docsDrawer: ReturnType<typeof useDocsDrawer>;
  handbookDrawer: ReturnType<typeof useHandbookDrawer>;
  setIsMenuOpen: (open: boolean) => void;
}) {
  if (isDocsPage && docsDrawer) {
    return (
      <button
        onClick={() => {
          if (!docsDrawer.isOpen) {
            setIsMenuOpen(false);
          }
          docsDrawer.setIsOpen(!docsDrawer.isOpen);
        }}
        className="flex h-8 cursor-pointer items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-3 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] md:hidden"
        aria-label={
          docsDrawer.isOpen ? "Close docs navigation" : "Open docs navigation"
        }
      >
        {docsDrawer.isOpen ? (
          <PanelLeftClose className="text-neutral-600" size={16} />
        ) : (
          <PanelLeft className="text-neutral-600" size={16} />
        )}
      </button>
    );
  }

  if (isHandbookPage && handbookDrawer) {
    return (
      <button
        onClick={() => {
          if (!handbookDrawer.isOpen) {
            setIsMenuOpen(false);
          }
          handbookDrawer.setIsOpen(!handbookDrawer.isOpen);
        }}
        className="flex h-8 cursor-pointer items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-3 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] md:hidden"
        aria-label={
          handbookDrawer.isOpen
            ? "Close handbook navigation"
            : "Open handbook navigation"
        }
      >
        {handbookDrawer.isOpen ? (
          <PanelLeftClose className="text-neutral-600" size={16} />
        ) : (
          <PanelLeft className="text-neutral-600" size={16} />
        )}
      </button>
    );
  }

  return null;
}

function Logo() {
  return (
    <Link
      to="/"
      className="mr-4 font-serif text-2xl font-semibold transition-transform hover:scale-105"
    >
      Char
    </Link>
  );
}

function ProductDropdown({
  isProductOpen,
  setIsProductOpen,
}: {
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
}) {
  return (
    <div
      className="relative hidden sm:block"
      onMouseEnter={() => setIsProductOpen(true)}
      onMouseLeave={() => setIsProductOpen(false)}
    >
      <button className="flex items-center gap-1 py-2 text-sm text-neutral-600 transition-all hover:text-neutral-800">
        Product
        {isProductOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isProductOpen && (
        <div className="absolute top-full left-0 z-50 w-150 pt-2">
          <div className="rounded-xs border border-neutral-200 bg-white py-2 shadow-lg">
            <div className="grid grid-cols-2 gap-x-6 px-3 py-2">
              <FeaturesList onClose={() => setIsProductOpen(false)} />
              <SolutionsList onClose={() => setIsProductOpen(false)} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function FeaturesList({ onClose }: { onClose: () => void }) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Features
      </div>
      {featuresList.map((link) => (
        <Link
          key={link.to}
          to={link.to}
          onClick={onClose}
          className="group flex items-center py-2 text-sm text-neutral-700"
        >
          <span className="decoration-dotted group-hover:underline">
            {link.label}
          </span>
        </Link>
      ))}
    </div>
  );
}

function SolutionsList({ onClose }: { onClose: () => void }) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Solutions
      </div>
      {solutionsList.map((link) => (
        <Link
          key={link.to}
          to={link.to}
          onClick={onClose}
          className="group flex items-center py-2 text-sm text-neutral-700"
        >
          <span className="decoration-dotted group-hover:underline">
            {link.label}
          </span>
        </Link>
      ))}
    </div>
  );
}

function ResourcesDropdown({
  isResourcesOpen,
  setIsResourcesOpen,
}: {
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
}) {
  return (
    <div
      className="relative hidden sm:block"
      onMouseEnter={() => setIsResourcesOpen(true)}
      onMouseLeave={() => setIsResourcesOpen(false)}
    >
      <button className="flex items-center gap-1 py-2 text-sm text-neutral-600 transition-all hover:text-neutral-800">
        Resources
        {isResourcesOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>
      {isResourcesOpen && (
        <div className="absolute top-full left-0 z-50 w-56 pt-2">
          <div className="rounded-xs border border-neutral-200 bg-white py-2 shadow-lg">
            <div className="px-3 py-2">
              {resourcesList.map((link) =>
                link.external ? (
                  <a
                    key={link.to}
                    href={link.to}
                    target="_blank"
                    rel="noopener noreferrer"
                    onClick={() => setIsResourcesOpen(false)}
                    className="group flex items-center gap-2 py-2 text-sm text-neutral-700"
                  >
                    <link.icon size={16} className="text-neutral-400" />
                    <span className="decoration-dotted group-hover:underline">
                      {link.label}
                    </span>
                  </a>
                ) : (
                  <Link
                    key={link.to}
                    to={link.to}
                    onClick={() => setIsResourcesOpen(false)}
                    className="group flex items-center gap-2 py-2 text-sm text-neutral-700"
                  >
                    <link.icon size={16} className="text-neutral-400" />
                    <span className="decoration-dotted group-hover:underline">
                      {link.label}
                    </span>
                  </Link>
                ),
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function NavLinks() {
  return (
    <Link
      to="/pricing/"
      className="hidden text-sm text-neutral-600 decoration-dotted transition-all hover:text-neutral-800 hover:underline sm:block"
    >
      Pricing
    </Link>
  );
}

function DesktopNav({
  platformCTA,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
}) {
  return (
    <nav className="hidden items-center gap-4 sm:flex">
      <SearchTrigger variant="header" />
      <CTAButton platformCTA={platformCTA} />
    </nav>
  );
}

function MobileNav({
  platform,
  platformCTA,
  isMenuOpen,
  setIsMenuOpen,
  docsDrawer,
  handbookDrawer,
  isDocsPage,
  isHandbookPage,
}: {
  platform: string;
  platformCTA: ReturnType<typeof getPlatformCTA>;
  isMenuOpen: boolean;
  setIsMenuOpen: (open: boolean) => void;
  docsDrawer: ReturnType<typeof useDocsDrawer>;
  handbookDrawer: ReturnType<typeof useHandbookDrawer>;
  isDocsPage: boolean;
  isHandbookPage: boolean;
}) {
  const hideCTA = isDocsPage || isHandbookPage;

  return (
    <div className="flex items-center gap-3 sm:hidden">
      {!hideCTA && (
        <div
          className={cn("transition-opacity duration-200 ease-out", [
            isMenuOpen ? "opacity-0" : "opacity-100",
          ])}
        >
          <CTAButton platformCTA={platformCTA} platform={platform} mobile />
        </div>
      )}
      <button
        onClick={() => {
          if (!isMenuOpen) {
            if (docsDrawer) {
              docsDrawer.setIsOpen(false);
            }
            if (handbookDrawer) {
              handbookDrawer.setIsOpen(false);
            }
          }
          setIsMenuOpen(!isMenuOpen);
        }}
        className="flex h-8 cursor-pointer items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-3 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
        aria-label={isMenuOpen ? "Close menu" : "Open menu"}
        aria-expanded={isMenuOpen}
      >
        {isMenuOpen ? (
          <X className="text-neutral-600" size={16} />
        ) : (
          <Menu className="text-neutral-600" size={16} />
        )}
      </button>
    </div>
  );
}

function CTAButton({
  platformCTA,
  platform,
  mobile = false,
}: {
  platformCTA: ReturnType<typeof getPlatformCTA>;
  platform?: string;
  mobile?: boolean;
}) {
  const baseClass = mobile
    ? "px-4 h-8 flex items-center text-sm bg-linear-to-t from-stone-600 to-stone-500 text-white rounded-full shadow-md active:scale-[98%] transition-all"
    : "px-4 h-8 flex items-center text-sm bg-linear-to-t from-stone-600 to-stone-500 text-white rounded-full shadow-md hover:shadow-lg hover:scale-[102%] active:scale-[98%] transition-all";

  if (mobile && platform === "mobile") {
    return (
      <Link to="/" hash="hero" onClick={scrollToHero} className={baseClass}>
        Get reminder
      </Link>
    );
  }

  if (platformCTA.action === "download") {
    return (
      <a href="/download/apple-silicon" download className={baseClass}>
        {platformCTA.label}
      </a>
    );
  }

  return (
    <Link to="/" hash="hero" onClick={scrollToHero} className={baseClass}>
      {platformCTA.label}
    </Link>
  );
}

function MobileMenu({
  isMenuOpen,
  setIsMenuOpen,
  isProductOpen,
  setIsProductOpen,
  isResourcesOpen,
  setIsResourcesOpen,
  platform,
  platformCTA,
  maxWidthClass,
}: {
  isMenuOpen: boolean;
  setIsMenuOpen: (open: boolean) => void;
  isProductOpen: boolean;
  setIsProductOpen: (open: boolean) => void;
  isResourcesOpen: boolean;
  setIsResourcesOpen: (open: boolean) => void;
  platform: string;
  platformCTA: ReturnType<typeof getPlatformCTA>;
  maxWidthClass: string;
}) {
  if (!isMenuOpen) return null;

  return (
    <>
      <div
        className="fixed inset-0 z-40 sm:hidden"
        onClick={() => setIsMenuOpen(false)}
      />
      <div className="animate-in slide-in-from-top fixed top-17.25 right-0 left-0 z-50 max-h-[calc(100vh-69px)] overflow-y-auto border-b border-neutral-100 bg-white/80 shadow-[0_4px_6px_-1px_rgba(0,0,0,0.1),0_2px_4px_-2px_rgba(0,0,0,0.1)] backdrop-blur-xs duration-300 sm:hidden">
        <nav className={`${maxWidthClass} mx-auto px-4 py-6`}>
          <div className="flex flex-col gap-6">
            <MobileMenuLinks
              isProductOpen={isProductOpen}
              setIsProductOpen={setIsProductOpen}
              isResourcesOpen={isResourcesOpen}
              setIsResourcesOpen={setIsResourcesOpen}
              setIsMenuOpen={setIsMenuOpen}
            />
            <MobileMenuCTAs
              platform={platform}
              platformCTA={platformCTA}
              setIsMenuOpen={setIsMenuOpen}
            />
          </div>
        </nav>
      </div>
    </>
  );
}

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
          <MobileFeaturesList setIsMenuOpen={setIsMenuOpen} />
          <MobileSolutionsList setIsMenuOpen={setIsMenuOpen} />
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
          {resourcesList.map((link) =>
            link.external ? (
              <a
                key={link.to}
                href={link.to}
                target="_blank"
                rel="noopener noreferrer"
                onClick={() => setIsMenuOpen(false)}
                className="flex items-center gap-2 py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
              >
                <link.icon size={14} className="text-neutral-400" />
                {link.label}
              </a>
            ) : (
              <Link
                key={link.to}
                to={link.to}
                onClick={() => setIsMenuOpen(false)}
                className="flex items-center gap-2 py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
              >
                <link.icon size={14} className="text-neutral-400" />
                {link.label}
              </Link>
            ),
          )}
        </div>
      )}
    </div>
  );
}

function MobileFeaturesList({
  setIsMenuOpen,
}: {
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Features
      </div>
      <div className="flex flex-col gap-2 pb-4">
        {featuresList.map((link) => (
          <Link
            key={link.to}
            to={link.to}
            onClick={() => setIsMenuOpen(false)}
            className="py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
          >
            {link.label}
          </Link>
        ))}
      </div>
    </div>
  );
}

function MobileSolutionsList({
  setIsMenuOpen,
}: {
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Solutions
      </div>
      <div className="flex flex-col gap-2">
        {solutionsList.map((link) => (
          <Link
            key={link.to}
            to={link.to}
            onClick={() => setIsMenuOpen(false)}
            className="py-1 text-sm text-neutral-600 transition-colors hover:text-neutral-900"
          >
            {link.label}
          </Link>
        ))}
      </div>
    </div>
  );
}

function BlogTocSubBar({
  blogToc,
  maxWidthClass,
}: {
  blogToc: NonNullable<ReturnType<typeof useBlogToc>>;
  maxWidthClass: string;
}) {
  const { toc, activeId, scrollToHeading } = blogToc;
  const activeIndex = toc.findIndex((item) => item.id === activeId);
  const activeItem = activeIndex >= 0 ? toc[activeIndex] : toc[0];

  const goPrev = () => {
    const prevIndex = Math.max(0, activeIndex - 1);
    scrollToHeading(toc[prevIndex].id);
  };

  const goNext = () => {
    const nextIndex = Math.min(toc.length - 1, activeIndex + 1);
    scrollToHeading(toc[nextIndex].id);
  };

  return (
    <div
      className={`${maxWidthClass} mx-auto border-x border-t border-neutral-100 border-t-neutral-50 sm:hidden`}
    >
      <div className="flex h-11 items-center px-2">
        <button
          onClick={goPrev}
          disabled={activeIndex <= 0}
          className={cn([
            "shrink-0 cursor-pointer rounded-md p-1.5 transition-colors",
            activeIndex <= 0
              ? "text-neutral-200"
              : "text-neutral-500 hover:bg-stone-50 hover:text-stone-700",
          ])}
        >
          <ChevronLeft size={14} />
        </button>
        <button
          onClick={() => {
            if (activeItem) scrollToHeading(activeItem.id);
          }}
          className="min-w-0 flex-1 cursor-pointer px-2"
        >
          <p className="truncate text-center text-sm font-medium text-stone-700">
            {activeItem?.text}
          </p>
        </button>
        <button
          onClick={goNext}
          disabled={activeIndex >= toc.length - 1}
          className={cn([
            "shrink-0 cursor-pointer rounded-md p-1.5 transition-colors",
            activeIndex >= toc.length - 1
              ? "text-neutral-200"
              : "text-neutral-500 hover:bg-stone-50 hover:text-stone-700",
          ])}
        >
          <ChevronRight size={14} />
        </button>
      </div>
    </div>
  );
}

function MobileMenuCTAs({
  platform,
  platformCTA,
  setIsMenuOpen,
}: {
  platform: string;
  platformCTA: ReturnType<typeof getPlatformCTA>;
  setIsMenuOpen: (open: boolean) => void;
}) {
  return (
    <div className="sticky bottom-4 flex flex-row gap-3">
      <Link
        to="/auth/"
        search={{ flow: "web" }}
        onClick={() => setIsMenuOpen(false)}
        className="block w-full rounded-lg border border-neutral-200 bg-white px-4 py-3 text-center text-sm text-neutral-700 transition-colors hover:bg-neutral-50"
      >
        Get started
      </Link>
      {platform === "mobile" ? (
        <Link
          to="/"
          hash="hero"
          onClick={() => {
            setIsMenuOpen(false);
            scrollToHero();
          }}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          Get reminder
        </Link>
      ) : platformCTA.action === "download" ? (
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
          hash="hero"
          onClick={() => {
            setIsMenuOpen(false);
            scrollToHero();
          }}
          className="block w-full rounded-lg bg-linear-to-t from-stone-600 to-stone-500 px-4 py-3 text-center text-sm text-white shadow-md transition-all active:scale-[98%]"
        >
          {platformCTA.label}
        </Link>
      )}
    </div>
  );
}
