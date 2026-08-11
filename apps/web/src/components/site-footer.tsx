import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { fetchUser } from "@/functions/auth";

const footerGroups = [
  {
    title: "Product",
    links: [
      { label: "Download", to: "/download/" },
      { label: "Enterprise", to: "/enterprise/" },
      { label: "Blog", to: "/blog/" },
      { label: "Changelog", to: "/changelog/" },
      { label: "Status", href: "https://status.anarlog.so" },
    ],
  },
  {
    title: "Community",
    links: [
      { label: "GitHub", href: "https://github.com/fastrepl/anarlog" },
      { label: "X", href: "https://x.com/anarlogapp" },
      { label: "Discord", to: "/discord/" },
    ],
  },
  {
    title: "Legal",
    links: [
      { label: "Privacy", to: "/privacy/" },
      { label: "Terms", to: "/terms/" },
    ],
  },
];

export function SiteFooter() {
  const { data: user } = useQuery({
    queryKey: ["auth-user"],
    queryFn: () => fetchUser(),
  });

  return (
    <footer className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-5 py-10 text-xs text-[#4f4940] sm:flex-row sm:justify-between md:px-8">
      <div className="flex flex-col gap-2.5 self-start">
        <a
          href="https://fastrepl.com"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 text-xs text-[#756b5d] opacity-75 transition-opacity hover:opacity-100"
        >
          <img
            src="/icons/fastrepl.svg"
            alt="Fastrepl"
            className="h-4 w-auto"
          />
          <span>© 2026</span>
        </a>
        {user ? (
          <Link to="/app/account/" className="w-fit hover:text-[#181613]">
            Account
          </Link>
        ) : (
          <Link
            to="/auth/"
            search={{ flow: "web" }}
            className="w-fit hover:text-[#181613]"
          >
            Get started
          </Link>
        )}
      </div>
      <nav className="grid grid-cols-2 gap-x-12 gap-y-8 sm:grid-cols-3 sm:gap-x-16">
        {footerGroups.map((group) => (
          <div key={group.title} className="flex flex-col gap-2.5">
            <span className="tracking-[0.04em] text-[#756b5d]">
              {group.title}
            </span>
            {group.links.map((link) =>
              link.to ? (
                <Link
                  key={link.label}
                  to={link.to}
                  className="w-fit hover:text-[#181613]"
                >
                  {link.label}
                </Link>
              ) : (
                <a
                  key={link.label}
                  href={link.href}
                  className="w-fit hover:text-[#181613]"
                >
                  {link.label}
                </a>
              ),
            )}
          </div>
        ))}
      </nav>
    </footer>
  );
}
