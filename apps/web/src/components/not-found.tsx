import { Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";

import { Footer } from "./footer";
import { Header } from "./header";

export function NotFoundContent() {
  return (
    <div className="flex min-h-[80vh] items-center justify-center">
      <div className="mx-auto flex w-full max-w-6xl items-center justify-center border-x border-neutral-100 bg-white px-4 py-32">
        <div className="text-center">
          <p className="mb-6 text-sm font-medium tracking-widest text-neutral-400 uppercase">
            404
          </p>

          <h1 className="mb-4 font-serif text-4xl text-neutral-900 sm:text-5xl">
            Page not found
          </h1>

          <p className="mx-auto mb-10 max-w-sm text-base text-neutral-500">
            The page you're looking for doesn't exist or has been moved.
          </p>

          <Link
            to="/"
            className="inline-flex items-center gap-2 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-5 py-2.5 text-sm font-medium text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
          >
            <ArrowLeft size={14} />
            Back to home
          </Link>
        </div>
      </div>
    </div>
  );
}

export function NotFoundDocument() {
  return (
    <div className="flex min-h-screen flex-col">
      <Header />
      <main className="flex-1">
        <NotFoundContent />
      </main>
      <Footer />
    </div>
  );
}
