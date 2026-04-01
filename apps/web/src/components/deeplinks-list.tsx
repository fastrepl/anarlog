import { MDXContent } from "@content-collections/mdx/react";
import { allDeeplinks } from "content-collections";

import { MDXLink, Mermaid, Tweet } from "@/components/mdx";

export function DeeplinksList() {
  const deeplinks = allDeeplinks.sort((a, b) => a.path.localeCompare(b.path));

  if (deeplinks.length === 0) {
    return null;
  }

  return (
    <div className="mt-6 flex flex-col gap-10">
      {deeplinks.map((deeplink) => (
        <section
          key={deeplink.slug}
          className="border-t pt-2 first:border-t-0 first:pt-0"
        >
          <div className="flex flex-col gap-1">
            <h2
              id={deeplink.path}
              className="scroll-mt-24 text-xl font-bold tracking-tight text-neutral-900"
            >
              {deeplink.path}
            </h2>
            {deeplink.description && (
              <p className="leading-relaxed text-neutral-600">
                {deeplink.description}
              </p>
            )}
          </div>

          {deeplink.params && deeplink.params.length > 0 && (
            <div className="mt-4 rounded-lg border border-neutral-200 bg-neutral-50/50 px-3 pt-2 pb-3 sm:px-4 sm:pt-3 sm:pb-4">
              <h3 className="mb-2 font-mono text-[10px] leading-none font-bold tracking-wider text-neutral-500/80 uppercase">
                Parameters
              </h3>
              <div className="flex flex-col gap-3">
                {deeplink.params.map((param) => (
                  <div
                    key={param.name}
                    className="group grid grid-cols-1 gap-1.5 sm:grid-cols-[180px_1fr] sm:gap-4"
                  >
                    <div className="flex flex-col items-start gap-1">
                      <code className="rounded bg-white px-1.5 py-0.5 font-mono text-xs font-semibold text-neutral-900 shadow-xs ring-1 ring-neutral-200">
                        {param.name}
                      </code>
                      <div className="flex items-center gap-1.5 px-0.5 font-mono text-[10px] text-neutral-500">
                        <span>{param.type_name}</span>
                      </div>
                    </div>
                    <div className="text-sm leading-relaxed text-neutral-600">
                      {param.description}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="prose prose-sm prose-neutral prose-headings:scroll-mt-24 prose-headings:font-semibold prose-p:leading-relaxed mt-4 max-w-none">
            <MDXContent
              code={deeplink.mdx}
              components={{
                a: MDXLink,
                mermaid: Mermaid,
                Mermaid,
                Tweet,
              }}
            />
          </div>
        </section>
      ))}
    </div>
  );
}
