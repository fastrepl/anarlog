import { Link } from "@tanstack/react-router";

export function SiteFooter() {
  return (
    <footer className="mx-auto flex w-full max-w-6xl flex-wrap items-center justify-between gap-5 px-5 py-8 text-xs text-[#4f4940] md:px-8">
      <a
        href="https://fastrepl.com"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1.5 text-xs text-[#756b5d] opacity-75 transition-opacity hover:opacity-100"
      >
        <img src="/icons/fastrepl.svg" alt="Fastrepl" className="h-4 w-auto" />
        <span>© 2026</span>
      </a>
      <nav className="flex flex-wrap gap-x-5 gap-y-2">
        <Link to="/download/" className="hover:text-[#181613]">
          Download
        </Link>
        <a
          href="https://github.com/fastrepl/anarlog"
          className="hover:text-[#181613]"
        >
          GitHub
        </a>
        <a href="https://x.com/anarlogapp" className="hover:text-[#181613]">
          X
        </a>
        <Link to="/discord/" className="hover:text-[#181613]">
          Discord
        </Link>
        <Link to="/blog/" className="hover:text-[#181613]">
          Blog
        </Link>
        <Link to="/changelog/" className="hover:text-[#181613]">
          Changelog
        </Link>
        <a href="https://status.anarlog.so" className="hover:text-[#181613]">
          Status
        </a>
        <Link to="/privacy/" className="hover:text-[#181613]">
          Privacy
        </Link>
        <Link to="/terms/" className="hover:text-[#181613]">
          Terms
        </Link>
      </nav>
    </footer>
  );
}
