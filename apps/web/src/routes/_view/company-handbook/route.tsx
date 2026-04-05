import { createFileRoute, Outlet, useMatchRoute } from "@tanstack/react-router";
import { allHandbooks } from "content-collections";
import { useMemo, useRef } from "react";

import { SidebarNavigation } from "@/components/sidebar-navigation";

import { handbookStructure } from "./-structure";

export const Route = createFileRoute("/_view/company-handbook")({
  component: Component,
});

function Component() {
  return (
    <div className="flex min-h-[calc(100vh-4rem)]">
      <HandbookNav />
      <div className="min-w-0 flex-1">
        <Outlet />
      </div>
    </div>
  );
}

function HandbookNav() {
  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/company-handbook/$/", fuzzy: true });

  const currentSlug = (
    match && typeof match !== "boolean"
      ? (match._splat as string)?.replace(/\/$/, "")
      : undefined
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
    <aside className="hidden w-56 shrink-0 md:block">
      <div
        ref={scrollContainerRef}
        className="scrollbar-hide sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto border-r border-neutral-100 px-4 py-6 xl:top-0 xl:h-screen"
      >
        <SidebarNavigation
          sections={handbooksBySection.sections}
          currentSlug={currentSlug}
          scrollContainerRef={scrollContainerRef}
          linkTo="/company-handbook/$/"
        />
      </div>
    </aside>
  );
}
