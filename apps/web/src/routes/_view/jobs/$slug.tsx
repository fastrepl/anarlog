import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, notFound } from "@tanstack/react-router";
import { allJobs } from "content-collections";
import { Children, isValidElement, type ReactNode } from "react";

import { AnimatedTitle } from "@/components/animated-title";
import { Image } from "@/components/image";
import { MDXLink } from "@/components/mdx";
import { jobsMdxComponents } from "@/components/mdx-jobs";

export const Route = createFileRoute("/_view/jobs/$slug")({
  beforeLoad: () => {
    if (!import.meta.env.DEV) {
      throw notFound();
    }
  },
  component: JobPage,
  loader: async ({ params }) => {
    const job = allJobs.find((j) => j.slug === params.slug);
    if (!job) {
      throw notFound();
    }
    if (!import.meta.env.DEV && job.published === false) {
      throw notFound();
    }
    return { job };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.job) {
      return { meta: [] };
    }

    const { job } = loaderData;

    const ogParams = new URLSearchParams({
      type: "jobs",
      title: job.title,
      backgroundImage: job.backgroundImage,
    });
    if (job.description) {
      ogParams.set("description", job.description);
    }
    const ogImageUrl = `/og?${ogParams.toString()}`;

    return {
      meta: [
        { title: `${job.title} - Char` },
        { name: "description", content: job.description },
        { property: "og:title", content: `${job.title} - Char` },
        { property: "og:description", content: job.description },
        { property: "og:image", content: ogImageUrl },
        { property: "og:type", content: "website" },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:title", content: `${job.title} - Char` },
        { name: "twitter:description", content: job.description },
        { name: "twitter:image", content: ogImageUrl },
      ],
    };
  },
});

function JobPage() {
  const { job } = Route.useLoaderData();

  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection job={job} />
        <JobDetailsSection job={job} />
        <CTASection job={job} />
      </div>
    </div>
  );
}

function getApplyUrl(job: (typeof allJobs)[0]) {
  return (
    job.applyUrl ||
    `mailto:founders@char.com?subject=Application for ${job.title}`
  );
}

function HeroSection({ job }: { job: (typeof allJobs)[0] }) {
  return (
    <div className="relative overflow-hidden">
      <Image
        src={job.backgroundImage}
        alt=""
        layout="fullWidth"
        priority={false}
        objectFit="cover"
        className="absolute inset-0 h-full w-full"
      />
      <div
        className="absolute inset-0"
        style={{
          backdropFilter: "blur(12px)",
          mask: "linear-gradient(to bottom, transparent 0%, transparent 50%, black 100%)",
          WebkitMask:
            "linear-gradient(to bottom, transparent 0%, transparent 50%, black 100%)",
        }}
      />
      <div className="absolute inset-0 bg-linear-to-b from-white/60 via-white/70 to-white" />
      <div className="relative px-6 py-24 lg:py-40">
        <div className="mx-auto max-w-3xl text-left">
          <AnimatedTitle
            text={job.title.toLowerCase()}
            className="mb-4 font-mono text-4xl tracking-tight text-stone-800 sm:text-5xl"
          />
          <p className="mb-8 flex items-center justify-center gap-3 font-mono text-sm text-neutral-600">
            full-time, remote
          </p>
          <a
            href={getApplyUrl(job)}
            className="inline-flex h-10 items-center justify-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
          >
            Apply now
          </a>
        </div>
      </div>
    </div>
  );
}

function JobDetailsSection({ job }: { job: (typeof allJobs)[0] }) {
  return (
    <div className="px-4 pb-16 lg:pb-24">
      <div className="mx-auto max-w-3xl">
        <MDXContent
          code={job.mdx}
          components={{
            a: MDXLink,
            h2: ({ children }) => (
              <h2 className="mt-12 mb-6 font-mono text-lg tracking-widest text-neutral-400 uppercase first:mt-0">
                {stripLinks(children)}
              </h2>
            ),
            h3: ({ children }) => (
              <h3 className="mt-6 mb-2 text-base font-semibold text-neutral-700">
                {children}
              </h3>
            ),
            p: ({ children }) => (
              <p className="mb-4 text-neutral-600">{children}</p>
            ),
            ul: ({ children }) => (
              <ul className="mb-4 list-disc space-y-2 pl-5 text-neutral-600 [&_ul]:mt-2 [&_ul]:mb-0">
                {children}
              </ul>
            ),
            ol: ({ children }) => (
              <ol className="mb-4 list-decimal space-y-2 pl-5 text-neutral-600 [&_ol]:mt-2 [&_ol]:mb-0">
                {children}
              </ol>
            ),
            li: ({ children }) => <li className="pl-1">{children}</li>,
            ...jobsMdxComponents,
          }}
        />
      </div>
    </div>
  );
}

function stripLinks(children: ReactNode): ReactNode {
  return Children.map(children, (child) => {
    if (!isValidElement(child)) {
      return child;
    }
    const props = child.props as { href?: string; children?: ReactNode };
    if (child.type === "a" || props.href) {
      return stripLinks(props.children);
    }
    return child;
  });
}

function CTASection({ job }: { job: (typeof allJobs)[0] }) {
  return (
    <section className="laptop:px-0 bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-left">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <Image
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-mono text-2xl sm:text-3xl">Interested?</h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          We'd love to hear from you.
        </p>
        <div className="pt-6">
          <a
            href={getApplyUrl(job)}
            className="flex h-12 items-center justify-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 text-base text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] sm:text-lg"
          >
            Apply now
          </a>
        </div>
      </div>
    </section>
  );
}
