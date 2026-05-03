import { Link } from "@tanstack/react-router";

export function SiteFooter() {
  return (
    <footer className="mx-auto grid w-full max-w-[700px] gap-5 px-5 py-8 text-sm text-[#4f4940] md:grid-cols-[1fr_auto_1fr] md:items-center md:px-8">
      <Link to="/" aria-label="Anarlog home" className="md:justify-self-start">
        <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
      </Link>
      <p className="text-xs text-[#756b5d] md:justify-self-center">
        Fastrepl © 2026
      </p>
      <nav className="flex flex-wrap gap-x-5 gap-y-2 md:justify-self-end">
        <a
          href="https://github.com/fastrepl/anarlog"
          className="hover:text-[#181613]"
        >
          GitHub
        </a>
        <a href="mailto:founders@fastrepl.com" className="hover:text-[#181613]">
          Contact
        </a>
      </nav>
    </footer>
  );
}
