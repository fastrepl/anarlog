import { ArrowUpRight, Check, CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { SiteFooter } from "@/components/site-footer";
import { fetchUser } from "@/functions/auth";
import { applyYcPerk, submitYcPerkRequest } from "@/functions/yc-perk";
import { getCanonicalUrl } from "@/lib/seo";
import { validateYcVerificationUrl, ycPerkRequestSchema } from "@/lib/yc-perk";

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    flexDirection: "column",
    backgroundColor: "#fff",
    color: "#181613",
  },
  style2: {
    marginInline: "auto",
    width: "100%",
    maxWidth: "700px",
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingTop: {
      default: "1rem",
      "@media (width >= 48rem)": "1rem",
    },
    paddingBottom: {
      default: "2rem",
      "@media (width >= 48rem)": "3rem",
    },
  },
  style3: {
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
    paddingBottom: {
      default: "5rem",
      "@media (width >= 48rem)": "6rem",
    },
    textAlign: "center",
  },
  style4: {
    display: "flex",
    justifyContent: "center",
  },
  style5: {
    height: {
      default: "2rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    width: "auto",
  },
  style6: {
    marginTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    display: "inline-flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#756b5d",
  },
  style7: {
    width: "1.25rem",
    height: "1.25rem",
    borderRadius: ".125rem",
  },
  style8: {
    marginInline: "auto",
    marginTop: "1.25rem",
    fontFamily: fonts.hand,
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "4.5rem",
    },
    lineHeight: {
      default: 0.98,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
  },
  style9: {
    marginInline: "auto",
    marginTop: "1.5rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style10: {
    marginInline: "auto",
    marginTop: "2rem",
  },
  style11: {
    marginInline: "auto",
    borderRadius: "3px",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#eadfce",
    backgroundColor: "#fffaf0",
    padding: "1.5rem",
    textAlign: "left",
    boxShadow: "0 18px 50px #4436241f",
  },
  style12: {
    display: "flex",
    width: "2.5rem",
    height: "2.5rem",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: radii.full,
    backgroundColor: "#d1fae5",
    color: "#047857",
  },
  style13: {
    marginTop: "1.25rem",
    fontSize: "1.25rem",
    lineHeight: "1.75rem",
    fontWeight: 600,
    letterSpacing: "-.025em",
  },
  style14: {
    marginTop: ".5rem",
    fontSize: "1rem",
    lineHeight: "1.75rem",
    color: "#4f4940",
  },
  style15: {
    marginTop: "1.25rem",
    display: "inline-flex",
    minHeight: "2.75rem",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: radii.full,
    backgroundColor: {
      default: "#181613",
      ":hover": "#363029",
    },
    paddingInline: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style16: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style17: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: 0,
    position: "absolute",
    overflow: "hidden",
  },
  style18: {
    pointerEvents: "none",
    position: "absolute",
    left: "-10000px",
  },
  style19: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style20: {
    marginInline: "auto",
    marginTop: ".5rem",
    display: "inline-flex",
    minHeight: "3rem",
    alignItems: "center",
    justifyContent: "center",
    gap: ".5rem",
    borderRadius: radii.full,
    backgroundColor: {
      default: "#181613",
      ":hover": "#363029",
    },
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    opacity: {
      default: null,
      ":disabled": 0.6,
    },
  },
  style21: {
    width: "1.25rem",
    height: "1.25rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  style22: {
    marginInline: "auto",
    marginTop: ".25rem",
    display: "inline-flex",
    width: "fit-content",
    alignItems: "center",
    gap: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    textDecorationLine: "underline",
    textDecorationColor: "#b8afa4",
    textUnderlineOffset: "4px",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter, display, content-visibility, overlay, pointer-events",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style23: {
    marginTop: ".375rem",
    paddingInline: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  verificationInput: {
    backgroundColor: "#fff",
    borderColor: {
      default: "#d8d3cc",
      ":focus": "#756b5d",
    },
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: "0 1px 2px rgb(0 0 0 / 0.05)",
      ":focus": "0 0 0 3px rgb(216 211 204 / 0.4)",
    },
    color: "#181613",
    fontSize: "1rem",
    minHeight: "3.25rem",
    outline: "none",
    paddingInline: "1.25rem",
    transitionDuration: ".15s",
    transitionProperty: "border-color, box-shadow",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    width: "100%",
    "::placeholder": {
      color: "#918a80",
    },
  },
  invalidVerificationInput: {
    borderColor: "#ef4444",
  },
});
const title = "YC founder perk · Anarlog";
const description =
  "YC founders get one year of Anarlog Pro free for private, bot-free meeting notes.";
const invalidVerificationMessages = {
  not_verified: "This YC link is no longer active.",
  email_missing: "Update your YC link to include your email.",
};
export const Route = createFileRoute("/yc/")({
  component: YcPerkPage,
  loader: async () => ({
    user: await fetchUser(),
  }),
  head: () => ({
    meta: [
      {
        title,
      },
      {
        name: "description",
        content: description,
      },
      {
        property: "og:title",
        content: title,
      },
      {
        property: "og:description",
        content: description,
      },
      {
        property: "og:url",
        content: getCanonicalUrl("/yc"),
      },
      {
        name: "twitter:title",
        content: title,
      },
      {
        name: "twitter:description",
        content: description,
      },
      {
        name: "twitter:url",
        content: getCanonicalUrl("/yc"),
      },
    ],
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/yc"),
      },
    ],
  }),
});
function YcPerkPage() {
  const { user } = Route.useLoaderData();
  const navigate = useNavigate();
  const requestMutation = useMutation({
    mutationFn: async (data: {
      verificationUrl: string;
      additionalComments: string;
    }) => {
      if (user) {
        return applyYcPerk({
          data: {
            value: data.verificationUrl,
          },
        });
      }
      return submitYcPerkRequest({
        data,
      });
    },
    onSuccess: (result) => {
      if (
        result.status === "needs_checkout" &&
        "code" in result &&
        result.code
      ) {
        void navigate({
          to: "/app/checkout/",
          search: {
            period: "monthly",
            trial: "false",
            source: "yc_perk",
            code: result.code,
          },
        });
      }
    },
  });
  const form = useForm({
    defaultValues: {
      verificationUrl: "",
      additionalComments: "",
    },
    validators: {
      onSubmit: ycPerkRequestSchema,
    },
    onSubmit: ({ value }) => requestMutation.mutate(value),
  });
  const appliedToAccount =
    requestMutation.data?.status === "applied" ||
    requestMutation.data?.status === "already_applied";
  const requestSucceeded =
    requestMutation.data?.status === "verified" ||
    requestMutation.data?.status === "submitted" ||
    appliedToAccount;
  const invalidVerificationMessage =
    requestMutation.data?.status === "invalid"
      ? invalidVerificationMessages[requestMutation.data.reason]
      : undefined;
  const requestErrorMessage =
    requestMutation.data?.status === "already_claimed" ||
    requestMutation.data?.status === "claimed"
      ? "This perk has already been claimed."
      : requestMutation.data?.status === "invalid_code"
        ? "This YC code is not valid."
        : invalidVerificationMessage;
  return (
    <div {...stylex.props(styles.style1)}>
      <main {...stylex.props(styles.style2)}>
        <section {...stylex.props(styles.style3)}>
          <Link
            to="/"
            aria-label="Anarlog home"
            {...stylex.props(styles.style4)}
          >
            <AnarlogLogo sx={styles.style5} />
          </Link>

          <div {...stylex.props(styles.style6)}>
            <img
              src="/icons/yc.svg"
              alt=""
              width={20}
              height={20}
              {...stylex.props(styles.style7)}
            />
            YC founder perk
          </div>

          <h1 {...stylex.props(styles.style8)}>
            Build the company. Keep every decision.
          </h1>
          <p {...stylex.props(styles.style9)}>
            Get 1 year of Anarlog Pro free for private, bot-free meeting notes.
          </p>

          <div {...stylex.props(styles.style10)}>
            {requestSucceeded ? (
              <div {...stylex.props(styles.style11)} role="status">
                <div {...stylex.props(styles.style12)}>
                  <Check size={20} weight="bold" aria-hidden="true" />
                </div>
                <h2 {...stylex.props(styles.style13)}>You’re verified.</h2>
                <p {...stylex.props(styles.style14)}>
                  {appliedToAccount
                    ? "Your YC year is on this account."
                    : "We sent your Pro code to your YC email."}
                </p>
                {appliedToAccount ? (
                  <Link to="/app/account/" {...stylex.props(styles.style15)}>
                    View account
                  </Link>
                ) : null}
              </div>
            ) : (
              <form
                {...stylex.props(styles.style16)}
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
                      <label
                        htmlFor={field.name}
                        {...stylex.props(styles.style17)}
                      >
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
                        {...stylex.props([
                          styles.verificationInput,
                          field.state.meta.errors.length > 0 &&
                            styles.invalidVerificationInput,
                        ])}
                        aria-invalid={field.state.meta.errors.length > 0}
                      />
                      <FieldError errors={field.state.meta.errors} />
                    </div>
                  )}
                </form.Field>

                <form.Field name="additionalComments">
                  {(field) => (
                    <div {...stylex.props(styles.style18)} aria-hidden="true">
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
                  <p {...stylex.props(styles.style19)} role="alert">
                    {requestErrorMessage ??
                      "We couldn’t process this. Try again."}
                  </p>
                )}

                <button
                  type="submit"
                  disabled={requestMutation.isPending}
                  {...stylex.props(styles.style20)}
                >
                  {requestMutation.isPending ? (
                    <>
                      <CircleNotch
                        {...stylex.props(styles.style21)}
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
                  {...stylex.props(styles.style22)}
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
    <p {...stylex.props(styles.style23)} role="alert">
      {message}
    </p>
  ) : null;
}
