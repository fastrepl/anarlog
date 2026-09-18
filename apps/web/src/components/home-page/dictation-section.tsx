import { Link } from "@tanstack/react-router";

import { ArrowRight, CheckCircle, Microphone } from "@anlg/ui/components/icons";

export function DictationSection() {
  return (
    <section id="dictation" className="py-16 md:py-20">
      <h2 className="text-brand-dark font-hand text-3xl leading-none font-semibold">
        Dictation, built right in
      </h2>
      <p className="text-color mx-auto mt-6 max-w-2xl text-lg leading-8">
        Your voice is useful beyond meetings. Turn a thought into an email,
        message, or document with dictation built into Anarlog.
      </p>

      <div className="border-color-subtle surface-subtle mt-8 rounded-3xl border p-5 text-left sm:p-8">
        <div className="text-color flex items-center gap-3 text-sm">
          <span className="bg-fg text-surface flex size-9 shrink-0 items-center justify-center rounded-full">
            <Microphone size={18} aria-hidden="true" />
          </span>
          <span>Speak your next follow-up</span>
        </div>
        <div className="surface border-color-subtle mt-5 rounded-2xl border p-5 sm:p-6">
          <p className="text-color-secondary text-xs">
            An email, a message, a doc
          </p>
          <p className="text-color mt-3 text-base leading-7 sm:text-lg">
            Thanks for the conversation today. I’ll send over the updated
            proposal tomorrow, and we can take it from there.
          </p>
          <div className="text-color-secondary mt-5 flex items-center gap-2 text-xs">
            <CheckCircle size={16} aria-hidden="true" />
            <span>From spoken thought to written text</span>
          </div>
        </div>
      </div>

      <p className="text-color mx-auto mt-6 max-w-xl text-base leading-7">
        Enable dictation, choose your shortcut, and speak into a text field in
        your desktop apps. Your words appear when you finish.
      </p>
      <p className="text-color-secondary mt-3 text-sm leading-6">
        Included with Pro, Team, and Enterprise on macOS, Windows, and Linux.
      </p>
      <Link
        to="/download/"
        className="bg-fg text-surface hover:bg-fg/90 mt-6 inline-flex items-center gap-2 rounded-full px-5 py-3 text-sm font-medium transition-colors"
      >
        Try dictation with Pro
        <ArrowRight size={16} aria-hidden="true" />
      </Link>
    </section>
  );
}
