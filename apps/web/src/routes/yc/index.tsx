import { ArrowUpRight, Check, CircleNotch } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { SiteFooter } from "@/components/site-footer";
import { submitYcPerkRequest } from "@/functions/yc-perk";
import { getCanonicalUrl } from "@/lib/seo";
import { validateYcVerificationUrl, ycPerkRequestSchema } from "@/lib/yc-perk";

const title = "YC founder perk · Anarlog";
const description =
  "YC founders get one year of Anarlog Pro free for private, bot-free meeting notes.";

const invalidVerificationMessages = {
  not_verified: "This YC link is no longer active.",
  email_missing: "Update your YC link to include your email.",
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
      verificationUrl: string;
      additionalComments: string;
    }) => submitYcPerkRequest({ data }),
  });
  const form = useForm({
    defaultValues: {
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
      ? "This perk has already been claimed."
      : invalidVerificationMessage;

  return (
    <div className="flex min-h-screen flex-col bg-white text-[#181613]">
      <main className="mx-auto w-full max-w-[700px] flex-1 px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <section className="pt-10 pb-20 text-center md:pt-12 md:pb-24">
          <Link
            to="/"
            aria-label="Anarlog home"
            className="flex justify-center"
          >
            <AnarlogLogo className="h-8 w-auto md:h-9" />
          </Link>

          <div className="mt-12 inline-flex items-center gap-2 text-sm font-medium text-[#756b5d] md:mt-16">
            <img
              src="/icons/yc.svg"
              alt=""
              width={20}
              height={20}
              className="size-5 rounded-sm"
            />
            YC founder perk
          </div>

          <h1 className="font-hand mx-auto mt-5 max-w-3xl text-5xl leading-[0.98] font-semibold tracking-normal text-balance md:text-7xl">
            Build the company. Keep every decision.
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
            Get 1 year of Anarlog Pro free for private, bot-free meeting notes.
          </p>

          <div className="mx-auto mt-8 max-w-xl">
            {requestSucceeded ? (
              <div
                className="mx-auto max-w-lg rounded-[3px] border border-[#eadfce] bg-[#fffaf0] p-6 text-left shadow-[0_18px_50px_rgba(68,54,36,0.12)]"
                role="status"
              >
                <div className="flex size-10 items-center justify-center rounded-full bg-emerald-100 text-emerald-700">
                  <Check size={20} weight="bold" aria-hidden="true" />
                </div>
                <h2 className="mt-5 text-xl font-semibold tracking-tight">
                  You’re verified.
                </h2>
                <p className="mt-2 text-base leading-7 text-[#4f4940]">
                  We sent your Pro code to your YC email.
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
                        placeholder="YC verification link"
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(event) =>
                          field.handleChange(event.target.value)
                        }
                        className={cn([
                          "min-h-13 w-full rounded-full border bg-white px-5 text-base text-[#181613] shadow-sm transition outline-none",
                          "placeholder:text-[#918a80] focus:border-[#756b5d] focus:ring-3 focus:ring-[#d8d3cc]/40",
                          field.state.meta.errors.length > 0
                            ? "border-red-500"
                            : "border-[#d8d3cc]",
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
                      "We couldn’t process this. Try again."}
                  </p>
                )}

                <button
                  type="submit"
                  disabled={requestMutation.isPending}
                  className="mx-auto mt-2 inline-flex min-h-12 items-center justify-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white transition-colors hover:bg-[#363029] disabled:cursor-not-allowed disabled:opacity-60"
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
                    "Claim YC perk"
                  )}
                </button>

                <a
                  href="https://www.ycombinator.com/verify"
                  target="_blank"
                  rel="noreferrer"
                  className="mx-auto mt-1 inline-flex w-fit items-center gap-1 text-sm text-[#756b5d] underline decoration-[#b8afa4] underline-offset-4 transition hover:text-[#181613]"
                >
                  Get verification link
                  <ArrowUpRight size={14} aria-hidden="true" />
                </a>
              </form>
            )}
          </div>
        </section>
      </main>
      <SiteFooter />
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
