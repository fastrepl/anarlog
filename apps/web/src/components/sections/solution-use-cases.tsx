export function SolutionUseCases({
  title,
  description,
  items,
}: {
  title: string;
  description: string;
  items: Array<{ title: string; description: string }>;
}) {
  return (
    <section className="surface border-color-brand mb-24 rounded-xl border px-8 pt-16 pb-8">
      <div>
        <h2 className="text-fg mb-4 text-left font-mono text-4xl">{title}</h2>
        <p className="text-fg mb-12 max-w-2xl text-left">{description}</p>
        <div className="grid gap-6 md:grid-cols-2">
          {items.map((useCase) => (
            <div
              key={useCase.title}
              className="border-color-brand surface-subtle rounded-xl border p-6"
            >
              <h3 className="mb-2 text-lg font-medium text-stone-700">
                {useCase.title}
              </h3>
              <p className="text-sm leading-relaxed text-neutral-600">
                {useCase.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
