import { cn } from "@hypr/utils";

export const authInputClassName = cn([
  "h-12 w-full rounded-xl border border-[#d9d1c5] bg-white px-4",
  "text-[#181613] placeholder:text-[#9a9082]",
  "transition-colors hover:border-[#b9ae9f]",
  "focus:border-[#181613] focus:ring-2 focus:ring-[#181613]/10 focus:outline-hidden",
]);

export const authPrimaryButtonClassName = cn([
  "flex h-12 w-full cursor-pointer items-center justify-center gap-3 rounded-full px-5",
  "bg-[#181613] text-sm font-medium text-white",
  "transition-colors hover:bg-[#4f4940]",
  "focus-visible:ring-2 focus-visible:ring-[#181613] focus-visible:ring-offset-2 focus-visible:outline-hidden",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const authSecondaryButtonClassName = cn([
  "flex h-12 w-full cursor-pointer items-center justify-center gap-3 rounded-full border border-[#d9d1c5] bg-white px-5",
  "text-sm font-medium text-[#181613]",
  "transition-colors hover:bg-[#f7f4ef]",
  "focus-visible:ring-2 focus-visible:ring-[#181613] focus-visible:ring-offset-2 focus-visible:outline-hidden",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const authNoticeClassName =
  "rounded-xl border border-[#e5ddcf] bg-[#f7f4ef] p-4 text-center";

const authPromises = [
  "Bot-free meeting capture",
  "Fully offline notes",
  "On-device or bring-your-own-key AI",
];

export function AuthShell({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto flex min-h-screen w-full max-w-[1180px] flex-col px-5 sm:px-8">
        <div className="grid flex-1 items-center gap-12 py-10 md:grid-cols-[minmax(0,1fr)_minmax(360px,440px)] lg:gap-20 lg:py-16">
          <section className="hidden md:block">
            <p className="font-hand text-2xl leading-none font-semibold text-[#756b5d]">
              Stay present. Keep the notes.
            </p>
            <h2 className="font-hand mt-5 max-w-[620px] text-6xl leading-[0.95] font-semibold tracking-normal text-balance lg:text-7xl">
              AI notepad for{" "}
              <mark className="bg-[#fff0b3] px-1 text-[#181613]">
                private meetings.
              </mark>
            </h2>
            <p className="mt-7 max-w-xl text-lg leading-8 text-[#4f4940]">
              Jot notes during the call. Anarlog turns them into an editable
              summary while your work stays in your hands.
            </p>

            <ul className="mt-9 grid gap-3 text-sm text-[#4f4940]">
              {authPromises.map((promise) => (
                <li key={promise} className="flex items-center gap-3">
                  <span className="size-1.5 rounded-full bg-[#d1a321]" />
                  {promise}
                </li>
              ))}
            </ul>
          </section>

          <section className="mx-auto w-full max-w-[440px] overflow-hidden rounded-[24px] border border-[#e5ddcf] bg-white shadow-[0_24px_80px_rgba(24,22,19,0.10)]">
            <header className="border-b border-[#ede7dc] px-6 py-7 sm:px-8 sm:py-8">
              <p className="font-hand text-xl leading-none font-semibold text-[#756b5d]">
                Private by default
              </p>
              <h1 className="font-hand mt-3 text-4xl leading-none font-semibold text-[#181613]">
                {title}
              </h1>
              <p className="mt-3 text-sm leading-6 text-[#756b5d]">
                {description}
              </p>
            </header>

            <div className="p-6 sm:p-8">{children}</div>
          </section>
        </div>

        <footer className="flex min-h-16 items-center justify-between gap-4 border-t border-[#ede7dc] py-5 text-xs text-[#8b8174]">
          <span>Open source. Local-first. Yours.</span>
          <span className="hidden sm:inline">
            Your notes should survive us.
          </span>
        </footer>
      </div>
    </main>
  );
}
