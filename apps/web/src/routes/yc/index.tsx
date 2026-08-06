import {
  ArrowUpRight,
  Check,
  CircleNotch,
  LockKey,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { submitYcPerkRequest } from "@/functions/yc-perk";
import { getCanonicalUrl } from "@/lib/seo";
import {
  validateYcPerkEmail,
  validateYcVerificationUrl,
  ycPerkRequestSchema,
} from "@/lib/yc-perk";

const title = "YC founder perk · Anarlog";
const description =
  "YC founders get three months of Anarlog Pro free for bot-free, local-first meeting notes.";

const invalidVerificationMessages = {
  not_verified: "This YC verification link is no longer active.",
  email_missing:
    "Update your YC verification link to include your email, then try again.",
  email_mismatch:
    "Use the same email shown on your YC verification link, then try again.",
};

export const Route = createFileRoute("/yc/")({
  component: YcPerkPage,
  head: () => ({
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: getCanonicalUrl("/yc") },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:url", content: getCanonicalUrl("/yc") },
    ],
    links: [{ rel: "canonical", href: getCanonicalUrl("/yc") }],
  }),
});

function YcPerkPage() {
  const requestMutation = useMutation({
    mutationFn: (data: {
      email: string;
      verificationUrl: string;
      additionalComments: string;
    }) => submitYcPerkRequest({ data }),
  });
  const form = useForm({
    defaultValues: {
      email: "",
      verificationUrl: "",
      additionalComments: "",
    },
    validators: { onSubmit: ycPerkRequestSchema },
    onSubmit: ({ value }) => requestMutation.mutate(value),
  });
  const requestSucceeded =
    requestMutation.data?.status === "verified" ||
    requestMutation.data?.status === "submitted";
  const invalidVerificationMessage =
    requestMutation.data?.status === "invalid"
      ? invalidVerificationMessages[requestMutation.data.reason]
      : undefined;
  const requestErrorMessage =
    requestMutation.data?.status === "already_claimed"
      ? "This YC perk has already been claimed for this email."
      : invalidVerificationMessage;

  return (
    <main className="bg-page text-color min-h-screen">
      <header className="mx-auto flex w-full max-w-6xl items-center justify-between px-5 py-5 sm:px-8 lg:px-10">
        <Link to="/" aria-label="Anarlog home" className="inline-flex">
          <AnarlogLogo className="h-8 w-auto" />
        </Link>
        <div className="flex items-center gap-3">
          <span className="text-color-secondary hidden text-sm sm:inline">
            Built by YC founders
          </span>
          <img
            src="/icons/yc.svg"
            alt="Y Combinator"
            width={28}
            height={28}
            className="size-7 rounded"
          />
        </div>
      </header>

      <section className="mx-auto grid min-h-[calc(100vh-92px)] w-full max-w-6xl items-center gap-14 px-5 py-12 sm:px-8 md:py-16 lg:grid-cols-[minmax(0,0.92fr)_minmax(420px,1.08fr)] lg:gap-20 lg:px-10 lg:py-20">
        <div className="max-w-xl">
          <div className="brand-yellow border-color-subtle inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm font-medium">
            <img
              src="/icons/yc.svg"
              alt=""
              width={20}
              height={20}
              className="size-5 rounded-sm"
            />
            The YC founder offer
          </div>

          <h1 className="mt-8 text-5xl leading-[0.98]! font-medium tracking-[-0.045em] text-balance sm:text-6xl lg:text-7xl">
            Build the company. Keep every decision.
          </h1>
          <p className="text-color-secondary mt-7 max-w-lg text-lg leading-8">
            Get three months of Anarlog Pro free. Keep private, bot-free notes
            from customer calls, investor conversations, and the meetings that
            move your startup forward.
          </p>

          <div className="mt-9 max-w-lg">
            {requestSucceeded ? (
              <div
                className="surface border-color-subtle rounded-2xl border p-6 shadow-sm"
                role="status"
              >
                <div className="flex size-10 items-center justify-center rounded-full bg-emerald-100 text-emerald-700">
                  <Check size={20} weight="bold" aria-hidden="true" />
                </div>
                <h2 className="mt-5 text-xl font-semibold tracking-tight">
                  You’re verified.
                </h2>
                <p className="text-color-secondary mt-2 text-base leading-7">
                  Your three-month Pro code is on its way to{" "}
                  {requestMutation.variables?.email}.
                </p>
              </div>
            ) : (
              <form
                className="flex flex-col gap-3"
                onSubmit={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  void form.handleSubmit();
                }}
              >
                <form.Field
                  name="email"
                  validators={{
                    onChange: ({ value }) => validateYcPerkEmail(value),
                    onBlur: ({ value }) => validateYcPerkEmail(value),
                    onSubmit: ({ value }) => validateYcPerkEmail(value),
                  }}
                >
                  {(field) => (
                    <div>
                      <label htmlFor={field.name} className="sr-only">
                        Work email
                      </label>
                      <input
                        id={field.name}
                        name={field.name}
                        type="email"
                        autoComplete="email"
                        required
                        placeholder="Work email"
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(event) =>
                          field.handleChange(event.target.value)
                        }
                        className={cn([
                          "surface text-color min-h-13 w-full rounded-xl border px-4 text-base shadow-sm transition outline-none",
                          "placeholder:text-color-muted focus:border-stone-500 focus:ring-3 focus:ring-stone-300/40",
                          field.state.meta.errors.length > 0
                            ? "border-red-500"
                            : "border-color-subtle",
                        ])}
                        aria-invalid={field.state.meta.errors.length > 0}
                      />
                      <FieldError errors={field.state.meta.errors} />
                    </div>
                  )}
                </form.Field>

                <form.Field
                  name="verificationUrl"
                  validators={{
                    onChange: ({ value }) => validateYcVerificationUrl(value),
                    onBlur: ({ value }) => validateYcVerificationUrl(value),
                    onSubmit: ({ value }) => validateYcVerificationUrl(value),
                  }}
                >
                  {(field) => (
                    <div>
                      <label htmlFor={field.name} className="sr-only">
                        YC verification link
                      </label>
                      <input
                        id={field.name}
                        name={field.name}
                        type="url"
                        autoComplete="url"
                        required
                        placeholder="https://www.ycombinator.com/verify/..."
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(event) =>
                          field.handleChange(event.target.value)
                        }
                        className={cn([
                          "surface text-color min-h-13 w-full rounded-xl border px-4 text-base shadow-sm transition outline-none",
                          "placeholder:text-color-muted focus:border-stone-500 focus:ring-3 focus:ring-stone-300/40",
                          field.state.meta.errors.length > 0
                            ? "border-red-500"
                            : "border-color-subtle",
                        ])}
                        aria-invalid={field.state.meta.errors.length > 0}
                      />
                      <FieldError errors={field.state.meta.errors} />
                    </div>
                  )}
                </form.Field>

                <form.Field name="additionalComments">
                  {(field) => (
                    <div
                      className="pointer-events-none absolute -left-[10000px]"
                      aria-hidden="true"
                    >
                      <label htmlFor={field.name}>Additional comments</label>
                      <input
                        id={field.name}
                        name={field.name}
                        type="text"
                        tabIndex={-1}
                        autoComplete="off"
                        value={field.state.value}
                        onChange={(event) =>
                          field.handleChange(event.target.value)
                        }
                      />
                    </div>
                  )}
                </form.Field>

                {(requestMutation.isError || requestErrorMessage) && (
                  <p className="text-sm text-red-700" role="alert">
                    {requestErrorMessage ??
                      "We couldn’t process your YC perk right now. Please try again."}
                  </p>
                )}

                <button
                  type="submit"
                  disabled={requestMutation.isPending}
                  className="mt-1 inline-flex min-h-13 items-center justify-center gap-2 rounded-xl bg-linear-to-t from-stone-600 to-stone-500 px-5 text-base font-medium text-white shadow-md transition hover:-translate-y-0.5 hover:shadow-lg disabled:translate-y-0 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {requestMutation.isPending ? (
                    <>
                      <CircleNotch
                        className="size-5 animate-spin"
                        aria-hidden="true"
                      />
                      Submitting…
                    </>
                  ) : (
                    "Claim your YC perk"
                  )}
                </button>

                <a
                  href="https://www.ycombinator.com/verify"
                  target="_blank"
                  rel="noreferrer"
                  className="text-color-secondary mt-1 inline-flex w-fit items-center gap-1 text-sm underline decoration-stone-400 underline-offset-4 transition hover:text-stone-900"
                >
                  Get your YC verification link
                  <ArrowUpRight size={14} aria-hidden="true" />
                </a>
              </form>
            )}
          </div>

          <div className="text-color-secondary mt-8 flex items-center gap-2 text-sm">
            <LockKey size={16} aria-hidden="true" />
            Verified directly with Y Combinator. Your link is only used to
            confirm YC status.
          </div>
        </div>

        <FounderMeetingVisual />
      </section>
    </main>
  );
}

function FounderMeetingVisual() {
  return (
    <div
      className="relative mx-auto flex aspect-square w-full max-w-[560px] items-center justify-center"
      aria-hidden="true"
    >
      <div className="brand-yellow absolute inset-[8%] rounded-full blur-3xl" />
      <div className="surface border-color-subtle absolute top-[10%] left-[14%] z-10 flex size-28 -rotate-8 flex-col justify-between rounded-3xl border p-4 shadow-[0_24px_55px_rgba(68,54,36,0.18)] sm:size-32">
        <img
          src="/icons/yc.svg"
          alt=""
          width={48}
          height={48}
          className="size-12 rounded-lg shadow-sm"
        />
        <span className="text-right text-sm font-semibold">S25</span>
      </div>
      <div className="surface border-color-subtle absolute top-[13%] right-[12%] size-28 rotate-9 rounded-3xl border p-4 shadow-[0_24px_55px_rgba(68,54,36,0.16)] sm:size-32">
        <span className="text-color-secondary text-xs font-medium tracking-[0.14em] uppercase">
          Today
        </span>
        <p className="mt-3 text-sm leading-5 font-semibold">
          Customer interview
        </p>
      </div>

      <div className="surface-dark relative z-20 mt-14 w-[82%] rotate-[-2deg] rounded-[2rem] p-6 text-white shadow-[0_36px_80px_rgba(68,54,36,0.28)] sm:p-8">
        <div className="flex items-center justify-between">
          <span className="text-sm font-semibold text-white">Anarlog</span>
          <div className="flex items-center gap-1.5">
            {[8, 14, 20, 12, 17, 9].map((height, index) => (
              <span
                key={`${height}-${index}`}
                className="w-1 rounded-full bg-stone-300"
                style={{ height }}
              />
            ))}
          </div>
        </div>
        <div className="mt-10">
          <p className="text-xs tracking-[0.15em] text-stone-400 uppercase">
            Founder sync
          </p>
          <h2 className="mt-2 text-2xl leading-tight font-medium tracking-tight text-white sm:text-3xl">
            Decisions, without another bot in the room.
          </h2>
        </div>
        <div className="mt-8 space-y-3">
          {[
            "Ship onboarding this week",
            "John owns customer follow-up",
            "Review pricing on Friday",
          ].map((item) => (
            <div
              key={item}
              className="flex items-center gap-3 rounded-xl bg-white/8 px-4 py-3"
            >
              <span className="size-1.5 shrink-0 rounded-full bg-stone-300" />
              <span className="text-sm text-stone-200">{item}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function FieldError({ errors }: { errors: Array<unknown> }) {
  const firstError = errors[0];
  const message =
    typeof firstError === "string"
      ? firstError
      : firstError && typeof firstError === "object" && "message" in firstError
        ? String(firstError.message)
        : undefined;

  return message ? (
    <p className="mt-1.5 px-1 text-sm text-red-700" role="alert">
      {message}
    </p>
  ) : null;
}
