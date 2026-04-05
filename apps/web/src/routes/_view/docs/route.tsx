import { createFileRoute, Outlet, useMatchRoute } from "@tanstack/react-router";
import { useRef } from "react";

import { SidebarNavigation } from "@/components/sidebar-navigation";

import { getDocsBySection } from "./-structure";

export const Route = createFileRoute("/_view/docs")({
  component: Component,
});

function Component() {
  return (
    <div className="flex min-h-[calc(100vh-4rem)]">
      <DocsNav />
      <div className="min-w-0 flex-1">
        <Outlet />
      </div>
    </div>
  );
}

function DocsNav() {
  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/docs/$/", fuzzy: true });

  const currentSlug = (
    match && typeof match !== "boolean"
      ? (match._splat as string)?.replace(/\/$/, "")
      : undefined
  ) as string | undefined;

  const { sections } = getDocsBySection();
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  return (
    <aside className="hidden w-56 shrink-0 md:block">
      <div
        ref={scrollContainerRef}
        className="scrollbar-hide sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto border-r border-neutral-100 px-4 py-6 xl:top-0 xl:h-screen"
      >
        <SidebarNavigation
          sections={sections}
          currentSlug={currentSlug}
          scrollContainerRef={scrollContainerRef}
          linkTo="/docs/$/"
        />
      </div>
    </aside>
  );
}
