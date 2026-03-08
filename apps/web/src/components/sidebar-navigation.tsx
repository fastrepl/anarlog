import { Link } from "@tanstack/react-router";
import { useEffect, useRef } from "react";

export function SidebarNavigation<T extends { slug: string; title: string }>({
  sections,
  currentSlug,
  onLinkClick,
  scrollContainerRef,
  linkTo,
}: {
  sections: { title: string; docs: T[] }[];
  currentSlug: string | undefined;
  onLinkClick?: () => void;
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
  linkTo: string;
}) {
  const activeLinkRef = useRef<HTMLAnchorElement>(null);

  useEffect(() => {
    if (activeLinkRef.current && scrollContainerRef?.current) {
      const container = scrollContainerRef.current;
      const activeLink = activeLinkRef.current;

      requestAnimationFrame(() => {
        const containerRect = container.getBoundingClientRect();
        const linkRect = activeLink.getBoundingClientRect();

        const scrollTop =
          activeLink.offsetTop -
          container.offsetTop -
          containerRect.height / 2 +
          linkRect.height / 2;

        container.scrollTop = scrollTop;
      });
    }
  }, [currentSlug, scrollContainerRef]);

  return (
    <nav className="flex flex-col gap-4">
      {sections.map((section) => (
        <div key={section.title}>
          <h3 className="mb-2 px-3 text-sm font-semibold text-neutral-700">
            {section.title}
          </h3>
          <div className="flex flex-col gap-0.5">
            {section.docs.map((doc) => {
              const isActive = currentSlug === doc.slug;
              return (
                <Link
                  key={doc.slug}
                  to={linkTo}
                  params={{ _splat: doc.slug }}
                  onClick={onLinkClick}
                  ref={isActive ? activeLinkRef : undefined}
                  className={`block rounded-xs py-1.5 pr-3 pl-5 text-sm transition-colors ${
                    isActive
                      ? "bg-neutral-100 font-medium text-stone-600"
                      : "text-neutral-600 hover:bg-neutral-50 hover:text-stone-600"
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
  );
}
