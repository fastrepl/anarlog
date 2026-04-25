import { createFileRoute } from "@tanstack/react-router";
import { allOssFriends } from "content-collections";
import { ArrowUpRightIcon } from "lucide-react";

export const Route = createFileRoute("/_view/oss-friends")({
  component: Component,
  head: () => ({
    meta: [
      { title: "OSS Friends - Char" },
      {
        name: "description",
        content: "Open source projects we like and recommend.",
      },
      { property: "og:title", content: "OSS Friends - Char" },
      {
        property: "og:description",
        content: "Open source projects we like and recommend.",
      },
    ],
  }),
});

function Component() {
  const projects = [...allOssFriends].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  return (
    <main className="mx-auto min-h-screen w-full max-w-6xl px-4 py-5 sm:px-6 lg:px-8">
      <PageHeader />
      <section className="divide-color-subtle border-color-subtle divide-y border-y">
        {projects.map((project) => (
          <article
            key={project.slug}
            className="grid gap-4 py-5 md:grid-cols-[220px_minmax(0,1fr)_auto] md:items-start"
          >
            <div>
              <h2 className="text-color text-xl">{project.name}</h2>
              <a
                href={project.github}
                target="_blank"
                rel="noreferrer"
                className="text-color-muted hover:text-color mt-1 inline-flex items-center gap-1 text-sm"
              >
                GitHub
                <ArrowUpRightIcon className="size-3.5" aria-hidden="true" />
              </a>
            </div>
            <p className="text-color-muted text-base">{project.description}</p>
            <a
              href={project.href}
              target="_blank"
              rel="noreferrer"
              className="text-color inline-flex items-center gap-1 font-mono text-sm hover:text-neutral-600"
            >
              Visit
              <ArrowUpRightIcon className="size-3.5" aria-hidden="true" />
            </a>
          </article>
        ))}
      </section>
    </main>
  );
}

function PageHeader() {
  return (
    <header className="border-color-subtle border-b py-10 sm:py-14">
      <a
        href="/"
        className="text-color-muted mb-8 inline-block font-mono text-sm"
      >
        Char
      </a>
      <h1 className="text-color text-4xl sm:text-5xl">OSS Friends</h1>
      <p className="text-color-muted mt-4 max-w-2xl text-lg">
        A simple list of open source projects worth knowing about.
      </p>
    </header>
  );
}
