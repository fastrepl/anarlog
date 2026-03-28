import { Icon } from "@iconify-icon/react";

export function SolutionFeatures({
  title,
  description,
  items,
}: {
  title: string;
  description: string;
  items: Array<{ icon: string; title: string; description: string }>;
}) {
  return (
    <section className="pt-24 pb-16">
      <div className="border-color-brand border-b px-8">
        <h2 className="text-fg mb-4 text-left font-mono text-4xl">{title}</h2>
        <p className="text-fg-muted mb-12 max-w-2xl text-left">{description}</p>
      </div>
      <div className="mx-auto px-8 py-16">
        <div className="grid gap-16 md:grid-cols-2 lg:grid-cols-3">
          {items.map((feature) => (
            <div key={feature.title} className="flex flex-col gap-3">
              <div className="surface flex size-12 items-center justify-center rounded-xl">
                <Icon icon={feature.icon} className="text-2xl text-stone-700" />
              </div>
              <h3 className="text-fg font-mono text-xl">{feature.title}</h3>
              <p className="text-fg w-4/5 text-sm leading-relaxed">
                {feature.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
