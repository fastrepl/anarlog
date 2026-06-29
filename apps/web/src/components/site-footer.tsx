import { Link } from "@tanstack/react-router";

export function SiteFooter() {
  return (
    <footer className="mx-auto flex w-full max-w-[700px] flex-wrap items-center justify-between gap-5 px-5 py-8 text-sm text-[#4f4940] md:px-8">
      <p className="text-sm text-[#756b5d]">Fastrepl © 2026</p>
      <nav className="flex flex-wrap gap-x-5 gap-y-2">
        <a
          href="https://github.com/fastrepl/anarlog"
          className="hover:text-[#181613]"
        >
          GitHub
        </a>
        <Link to="/blog/" className="hover:text-[#181613]">
          Blog
        </Link>
        <Link to="/changelog/" className="hover:text-[#181613]">
          Changelog
        </Link>
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
