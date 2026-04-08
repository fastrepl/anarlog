import { cn } from "@hypr/utils";

export function AcquisitionLinkGrid({
  title,
  description,
  items,
  className,
}: {
  title: string;
  description?: string;
  items: Array<{
    eyebrow?: string;
    title: string;
    description: string;
    href: string;
  }>;
  className?: string;
}) {
  if (items.length === 0) {
    return null;
  }

  return (
    <section className={cn(["border-color-brand border-t pt-8", className])}>
      <div className="">
        <div className="mb-8 flex flex-col gap-2 text-left">
          <h2 className="text-color font-sans text-2xl tracking-tight">
            {title}
          </h2>
          {description ? (
            <p className="text-color-secondary text-base leading-7 sm:text-lg">
              {description}
            </p>
          ) : null}
        </div>

        <div className="border-color-brand grid overflow-hidden rounded-2xl border md:grid-cols-3">
          {items.map((item, index) => {
            const cols = 3;
            const isLast = index === items.length - 1;
            const isRightEdge = (index + 1) % cols === 0;
            const isBottomRow =
              Math.floor(index / cols) === Math.ceil(items.length / cols) - 1;

            return (
              <a
                key={item.href}
                href={item.href}
                className={cn([
                  "group border-color-brand p-8 text-left transition-colors",
                  !isLast && "border-b",
                  isBottomRow && "md:border-b-0",
                  !isRightEdge && "md:border-r",
                  "hover:bg-white",
                ])}
              >
                {item.eyebrow ? (
                  <div className="mb-3 font-mono text-[11px] tracking-[0.18em] text-stone-500 uppercase">
                    {item.eyebrow}
                  </div>
                ) : null}
                <h3 className="text-color mb-2 font-mono text-lg font-medium">
                  {item.title}
                </h3>
                <p className="text-color-secondary text-sm leading-5">
                  {item.description}
                </p>
                <div className="mt-4 text-sm font-medium text-stone-700 transition-colors group-hover:text-stone-950">
                  Explore
                </div>
              </a>
            );
          })}
        </div>
      </div>
    </section>
  );
}
