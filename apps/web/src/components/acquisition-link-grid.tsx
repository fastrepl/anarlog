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
    <section className={cn(["border-t border-neutral-200 pt-8", className])}>
      <div className="max-w-6xl">
        <div className="mb-6 flex max-w-3xl flex-col gap-2 text-left">
          <h2 className="text-fg font-mono text-2xl tracking-tight sm:text-3xl">
            {title}
          </h2>
          {description ? (
            <p className="text-fg-muted text-base leading-7 sm:text-lg">
              {description}
            </p>
          ) : null}
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          {items.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className={cn([
                "group rounded-2xl border border-neutral-200 bg-white p-5 text-left transition-colors",
                "hover:border-stone-300 hover:bg-stone-50",
              ])}
            >
              {item.eyebrow ? (
                <div className="mb-3 font-mono text-[11px] tracking-[0.18em] text-stone-500 uppercase">
                  {item.eyebrow}
                </div>
              ) : null}
              <h3 className="text-fg mb-2 font-mono text-lg">{item.title}</h3>
              <p className="text-fg-muted text-sm leading-6">
                {item.description}
              </p>
              <div className="mt-4 text-sm font-medium text-stone-700 transition-colors group-hover:text-stone-950">
                Explore
              </div>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}
