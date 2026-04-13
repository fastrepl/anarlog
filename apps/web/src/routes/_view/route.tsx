import {
  createFileRoute,
  Link,
  Outlet,
  useMatchRoute,
  useRouterState,
} from "@tanstack/react-router";
import { allHandbooks } from "content-collections";
import { XIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { Footer } from "@/components/footer";
import { NotFoundContent } from "@/components/not-found";
import { RightPanel } from "@/components/right-panel";
import { SearchPaletteProvider } from "@/components/search";
import { Sidebar } from "@/components/sidebar";
import { SidebarNavigation } from "@/components/sidebar-navigation";
import { BlogTocContext } from "@/hooks/use-blog-toc";
import { DocsDrawerContext } from "@/hooks/use-docs-drawer";
import { HandbookDrawerContext } from "@/hooks/use-handbook-drawer";
import { HeroContext } from "@/hooks/use-hero-context";
import { brandPageNoiseBackgroundImage } from "@/lib/brand-noise";

import { handbookStructure } from "./company-handbook/-structure";
import { getDocsBySection } from "./docs/-structure";

export const Route = createFileRoute("/_view")({
  component: Component,
  notFoundComponent: NotFoundContent,
});

function Component() {
  const router = useRouterState();
  const pathname = router.location.pathname;
  const isDocsPage = pathname.startsWith("/docs");
  const isHandbookPage = pathname.startsWith("/company-handbook");
  const isChoosePage = pathname.startsWith("/choose");
  const isHomePage = pathname === "/";
  const hasHeroCTA =
    isHomePage || pathname.startsWith("/product/ai-notetaking");
  const isAppPage = pathname.startsWith("/app");
  const isResourcePage = [
    "/docs",
    "/blog",
    "/gallery",
    "/updates",
    "/changelog",
    "/roadmap",
    "/company-handbook",
  ].some((path) => pathname.startsWith(path));
  const [onTrigger, setOnTrigger] = useState<(() => void) | null>(null);
  const [isDocsDrawerOpen, setIsDocsDrawerOpen] = useState(false);
  const [isHandbookDrawerOpen, setIsHandbookDrawerOpen] = useState(false);
  const [blogToc, setBlogToc] = useState<
    Array<{ id: string; text: string; level: number }>
  >([]);
  const [blogActiveId, setBlogActiveId] = useState<string | null>(null);

  const scrollToHeading = useCallback((id: string) => {
    document.getElementById(id)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  }, []);

  return (
    <SearchPaletteProvider>
      <HeroContext.Provider
        value={{
          onTrigger,
          setOnTrigger: (callback) => setOnTrigger(() => callback),
        }}
      >
        <BlogTocContext.Provider
          value={{
            toc: blogToc,
            activeId: blogActiveId,
            setToc: setBlogToc,
            setActiveId: setBlogActiveId,
            scrollToHeading,
          }}
        >
          <DocsDrawerContext.Provider
            value={{
              isOpen: isDocsDrawerOpen,
              setIsOpen: setIsDocsDrawerOpen,
            }}
          >
            <HandbookDrawerContext.Provider
              value={{
                isOpen: isHandbookDrawerOpen,
                setIsOpen: setIsHandbookDrawerOpen,
              }}
            >
              <div className="relative flex min-h-screen flex-col">
                {isHomePage && <AnnouncementBanner />}
                {!isResourcePage && !isAppPage && (
                  <>
                    <div
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-x-0 top-0 z-0 h-[180vh]"
                      style={{
                        background:
                          "linear-gradient(to bottom, var(--brand-yellow), transparent)",
                      }}
                    />
                    <div
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-x-0 top-0 z-0 h-[180vh] opacity-30"
                      style={{
                        backgroundImage: brandPageNoiseBackgroundImage,
                        backgroundRepeat: "repeat",
                        maskImage:
                          "linear-gradient(to bottom, black, transparent)",
                        WebkitMaskImage:
                          "linear-gradient(to bottom, black, transparent)",
                      }}
                    />
                  </>
                )}

                {/* Mobile top bar spacer */}
                <div
                  className="xl:hidden"
                  style={{
                    height: "calc(3.5rem + var(--announcement-bar-h, 0px))",
                  }}
                />

                {/* Sidebar + content in a centered container */}
                <div className="relative z-10 mx-auto flex w-full max-w-[1800px]">
                  {!isChoosePage && <Sidebar />}
                  <main className="min-w-0 flex-1">
                    <Outlet />
                  </main>
                  {!isChoosePage && !isDocsPage && !isHandbookPage && (
                    <RightPanel revealCtaOnScroll={hasHeroCTA} />
                  )}
                </div>

                {!isChoosePage && <Footer />}

                {isDocsPage && (
                  <MobileDocsDrawer
                    isOpen={isDocsDrawerOpen}
                    onClose={() => setIsDocsDrawerOpen(false)}
                  />
                )}
                {isHandbookPage && (
                  <MobileHandbookDrawer
                    isOpen={isHandbookDrawerOpen}
                    onClose={() => setIsHandbookDrawerOpen(false)}
                  />
                )}
              </div>
            </HandbookDrawerContext.Provider>
          </DocsDrawerContext.Provider>
        </BlogTocContext.Provider>
      </HeroContext.Provider>
    </SearchPaletteProvider>
  );
}

function MobileDocsDrawer({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/docs/$/", fuzzy: true });

  const currentSlug = (
    match && typeof match !== "boolean" ? match._splat : undefined
  ) as string | undefined;

  const { sections } = getDocsBySection();
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  return (
    <>
      {isOpen && (
        <div
          className="fixed inset-0 top-14 z-40 xl:hidden"
          onClick={onClose}
        />
      )}
      <div
        className={`fixed top-14 left-0 z-50 h-[calc(100dvh-56px)] w-72 border-r border-neutral-100 bg-white/80 shadow-2xl shadow-neutral-900/20 backdrop-blur-xs transition-transform duration-300 ease-in-out xl:hidden ${
          isOpen ? "translate-x-0" : "-translate-x-full"
        }`}
        style={{
          paddingLeft: "env(safe-area-inset-left)",
        }}
      >
        <div
          ref={scrollContainerRef}
          className="scrollbar-hide h-full overflow-y-auto p-4"
        >
          <SidebarNavigation
            sections={sections}
            currentSlug={currentSlug}
            onLinkClick={onClose}
            scrollContainerRef={scrollContainerRef}
            linkTo="/docs/$/"
          />
        </div>
      </div>
    </>
  );
}

function MobileHandbookDrawer({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/company-handbook/$/", fuzzy: true });

  const currentSlug = (
    match && typeof match !== "boolean" ? match._splat : undefined
  ) as string | undefined;

  const handbooksBySection = useMemo(() => {
    const sectionGroups: Record<
      string,
      { title: string; docs: (typeof allHandbooks)[0][] }
    > = {};

    allHandbooks.forEach((doc) => {
      if (doc.slug === "index" || doc.isIndex) {
        return;
      }

      const sectionName = doc.section;

      if (!sectionGroups[sectionName]) {
        sectionGroups[sectionName] = {
          title: sectionName,
          docs: [],
        };
      }

      sectionGroups[sectionName].docs.push(doc);
    });

    Object.keys(sectionGroups).forEach((sectionName) => {
      sectionGroups[sectionName].docs.sort((a, b) => a.order - b.order);
    });

    const sections = handbookStructure.sections
      .map((sectionId) => {
        const sectionName = handbookStructure.sectionTitles[sectionId];
        return sectionGroups[sectionName];
      })
      .filter(Boolean);

    return { sections };
  }, []);

  const scrollContainerRef = useRef<HTMLDivElement>(null);

  return (
    <>
      {isOpen && (
        <div
          className="fixed inset-0 top-14 z-40 xl:hidden"
          onClick={onClose}
        />
      )}
      <div
        className={`fixed top-14 left-0 z-50 h-[calc(100dvh-56px)] w-72 border-r border-neutral-100 bg-white/80 shadow-2xl shadow-neutral-900/20 backdrop-blur-xs transition-transform duration-300 ease-in-out xl:hidden ${
          isOpen ? "translate-x-0" : "-translate-x-full"
        }`}
        style={{
          paddingLeft: "env(safe-area-inset-left)",
        }}
      >
        <div
          ref={scrollContainerRef}
          className="scrollbar-hide h-full overflow-y-auto p-4"
        >
          <SidebarNavigation
            sections={handbooksBySection.sections}
            currentSlug={currentSlug}
            onLinkClick={onClose}
            scrollContainerRef={scrollContainerRef}
            linkTo="/company-handbook/$/"
          />
        </div>
      </div>
    </>
  );
}

const ANNOUNCEMENT_STORAGE_KEY = "char_announcement_dismissed";

const ANNOUNCEMENT_BAR_HEIGHT = "36px";

function AnnouncementBanner() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const isDismissed =
      window.localStorage.getItem(ANNOUNCEMENT_STORAGE_KEY) === "true";
    if (!isDismissed) {
      setVisible(true);
    }
  }, []);

  useEffect(() => {
    document.documentElement.style.setProperty(
      "--announcement-bar-h",
      visible ? ANNOUNCEMENT_BAR_HEIGHT : "0px",
    );
  }, [visible]);

  if (!visible) return null;

  return (
    <>
      <Link
        to="/blog/$slug/"
        params={{ slug: "hyprnote-is-now-char" }}
        className={cn([
          "fixed inset-x-0 top-0 z-[60] flex items-center justify-center",
          "bg-stone-800 px-10 py-2",
          "font-serif text-sm text-stone-200",
          "transition-colors hover:bg-stone-700",
        ])}
      >
        <span>
          Hyprnote is now <strong>Char</strong>.
        </span>
        <button
          type="button"
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            window.localStorage.setItem(ANNOUNCEMENT_STORAGE_KEY, "true");
            setVisible(false);
          }}
          className="absolute right-4 cursor-pointer text-stone-400 transition-colors hover:text-white"
        >
          <XIcon size={14} />
        </button>
      </Link>
      <div style={{ height: ANNOUNCEMENT_BAR_HEIGHT }} />
    </>
  );
}
