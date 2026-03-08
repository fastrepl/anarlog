import {
  createFileRoute,
  Outlet,
  useMatchRoute,
  useRouterState,
} from "@tanstack/react-router";
import { allHandbooks } from "content-collections";
import { useCallback, useMemo, useRef, useState } from "react";

import { Footer } from "@/components/footer";
import { Header } from "@/components/header";
import { NotFoundContent } from "@/components/not-found";
import { SearchPaletteProvider } from "@/components/search";
import { SidebarNavigation } from "@/components/sidebar-navigation";
import { BlogTocContext } from "@/hooks/use-blog-toc";
import { DocsDrawerContext } from "@/hooks/use-docs-drawer";
import { HandbookDrawerContext } from "@/hooks/use-handbook-drawer";
import { HeroContext } from "@/hooks/use-hero-context";

import { handbookStructure } from "./company-handbook/-structure";
import { getDocsBySection } from "./docs/-structure";

export const Route = createFileRoute("/_view")({
  component: Component,
  notFoundComponent: NotFoundContent,
});

function Component() {
  const router = useRouterState();
  const isDocsPage = router.location.pathname.startsWith("/docs");
  const isHandbookPage =
    router.location.pathname.startsWith("/company-handbook");
  const isChoosePage = router.location.pathname.startsWith("/choose");
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
              <div className="flex min-h-screen flex-col">
                {!isChoosePage && <Header />}
                <main className="flex-1">
                  <Outlet />
                </main>
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
          className="fixed inset-0 top-17.25 z-40 md:hidden"
          onClick={onClose}
        />
      )}
      <div
        className={`fixed top-17.25 left-0 z-50 h-[calc(100vh-69px)] w-72 border-r border-neutral-100 bg-white/80 shadow-2xl shadow-neutral-900/20 backdrop-blur-xs transition-transform duration-300 ease-in-out md:hidden ${
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
          className="fixed inset-0 top-17.25 z-40 md:hidden"
          onClick={onClose}
        />
      )}
      <div
        className={`fixed top-17.25 left-0 z-50 h-[calc(100vh-69px)] w-72 border-r border-neutral-100 bg-white/80 shadow-2xl shadow-neutral-900/20 backdrop-blur-xs transition-transform duration-300 ease-in-out md:hidden ${
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
