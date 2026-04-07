import {
  createFileRoute,
  Link,
  Outlet,
  useMatchRoute,
} from "@tanstack/react-router";
import { useEffect, useRef } from "react";

import { getLegalSections } from "./-structure";

export const Route = createFileRoute("/_view/legal")({
  component: Component,
});

function Component() {
  return (
    <div className="flex min-h-[calc(100vh-4rem)]">
      <LegalNav />
      <div className="min-w-0 flex-1">
        <Outlet />
      </div>
    </div>
  );
}

function LegalNav() {
  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/legal/$slug/", fuzzy: true });
  const currentSlug =
    match && typeof match !== "boolean" ? (match.slug as string) : undefined;

  const sections = getLegalSections();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const activeLinkRef = useRef<HTMLAnchorElement>(null);

  useEffect(() => {
    if (activeLinkRef.current && scrollContainerRef.current) {
      const container = scrollContainerRef.current;
      const activeLink = activeLinkRef.current;

      requestAnimationFrame(() => {
        const containerRect = container.getBoundingClientRect();
        const linkRect = activeLink.getBoundingClientRect();

        container.scrollTop =
          activeLink.offsetTop -
          container.offsetTop -
          containerRect.height / 2 +
          linkRect.height / 2;
      });
    }
  }, [currentSlug]);

  return (
    <aside className="hidden w-56 shrink-0 md:block">
      <div
        ref={scrollContainerRef}
        className="scrollbar-hide sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto px-4 py-6 xl:top-0 xl:h-screen"
      >
        <nav className="flex flex-col gap-4">
          {sections.map((section) => (
            <div key={section.title}>
              <h3 className="text-fg mb-2 px-3 text-sm font-semibold">
                {section.title}
              </h3>
              <div className="flex flex-col gap-0.5">
                {section.docs.map((doc) => {
                  const isActive = currentSlug === doc.slug;
                  return (
                    <Link
                      key={doc.slug}
                      to="/legal/$slug/"
                      params={{ slug: doc.slug }}
                      ref={isActive ? activeLinkRef : undefined}
                      className={`block rounded-xs py-1.5 pr-3 pl-5 text-sm transition-colors ${
                        isActive
                          ? "text-fg font-medium underline decoration-dotted underline-offset-4"
                          : "text-fg opacity-50 hover:underline hover:decoration-dotted hover:underline-offset-4 hover:opacity-100"
                      }`}
                    >
                      {doc.title}
                    </Link>
                  );
                })}
              </div>
            </div>
          ))}
        </nav>
      </div>
    </aside>
  );
}
