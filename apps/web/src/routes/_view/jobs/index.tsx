import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allJobs } from "content-collections";
import { ArrowRight } from "lucide-react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";

export const Route = createFileRoute("/_view/jobs/")({
  beforeLoad: () => {
    if (!import.meta.env.DEV) {
      throw notFound();
    }
  },
  component: JobsPage,
  head: () => ({
    meta: [
      { title: "Jobs - Char" },
      {
        name: "description",
        content: "Join the Char team. View open positions and apply.",
      },
    ],
  }),
});

function JobsPage() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <JobsSection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="px-4 py-16 lg:py-24">
      <div className="mx-auto max-w-3xl text-left">
        <h1 className="text-color mb-6 font-mono text-4xl tracking-tight sm:text-5xl">
          Jobs
        </h1>
        <p className="text-fg-muted text-lg sm:text-xl">
          Join us in building the future of note-taking. We're a small team with
          big ambitions.
        </p>
      </div>
    </div>
  );
}

function JobsSection() {
  const jobs = allJobs.filter(
    (j) => import.meta.env.DEV || j.published !== false,
  );

  return (
    <section className="pb-16 lg:pb-24">
      {jobs.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2">
          {jobs.map((job, index) => (
            <JobCard key={job.slug} job={job} hasBorder={index === 0} />
          ))}
        </div>
      ) : (
        <div className="px-4 py-16 text-left">
          <p className="text-fg-muted text-lg">
            There are no open positions at the moment.
          </p>
        </div>
      )}
      <CTASection />
    </section>
  );
}

function JobCard({
  job,
  hasBorder,
}: {
  job: (typeof allJobs)[0];
  hasBorder?: boolean;
}) {
  return (
    <Link
      to="/jobs/$slug/"
      params={{ slug: job.slug }}
      className={cn([
        "group relative block overflow-hidden p-8 text-left",
        "transition-colors hover:bg-stone-50/50",
        hasBorder && "border-b border-neutral-100 md:border-r md:border-b-0",
      ])}
    >
      <div
        className="absolute inset-0 bg-cover bg-center"
        style={{ backgroundImage: `url(${job.backgroundImage})` }}
      />
      <div className="absolute inset-0 bg-linear-to-b from-white/95 via-white/90 to-white/95" />
      <div className="relative z-10 flex h-full flex-col">
        <h2 className="mb-2 text-xl font-medium text-stone-700">{job.title}</h2>
        <div className="mb-4 flex items-center gap-3 text-sm text-neutral-500">
          <span>Full-time</span>
          <span className="text-neutral-300">|</span>
          <span>Remote</span>
        </div>
        <p className="mb-6 leading-relaxed text-neutral-600">
          {job.cardDescription}
        </p>
        <div className="mt-auto">
          <span className="inline-flex h-8 items-center gap-2 rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm font-medium text-neutral-700 shadow-xs transition-all group-hover:scale-[102%] group-hover:shadow-md group-active:scale-[98%]">
            Interested?
            <ArrowRight className="size-4" />
          </span>
        </div>
      </div>
    </Link>
  );
}

function CTASection() {
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
        <h2 className="text-color font-mono text-2xl sm:text-3xl">
          Don't see a role that fits?
        </h2>
        <p className="text-fg-muted mx-auto max-w-2xl text-lg">
          We'd still love to hear from you!
        </p>
        <div className="pt-6">
          <a
            href="mailto:founders@char.com"
            className="flex h-12 items-center justify-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 text-base text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] sm:text-lg"
          >
            Contact us
          </a>
        </div>
      </div>
    </section>
  );
}
