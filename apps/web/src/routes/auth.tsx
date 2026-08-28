import { Icon } from "@iconify-icon/react";
import { ArrowLeft, Buildings, Envelope } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, redirect } from "@tanstack/react-router";
import { useRef, useState, type ReactNode } from "react";
import { z } from "zod";

import { radii } from "@anlg/design-system/tokens.stylex";

import { AuthShell, authStyles } from "@/components/auth-shell";
import {
  createDesktopSession,
  doAuth,
  doMagicLinkAuth,
  doPasswordSignIn,
  doPasswordSignUp,
  doSsoAuth,
  fetchUser,
} from "@/functions/auth";
import {
  DEFAULT_DESKTOP_SCHEME,
  type DesktopScheme,
  flowSearchSchema,
} from "@/functions/desktop-flow";
import { useMountEffect } from "@/hooks/useMountEffect";
import { toAuthFlowSearch } from "@/lib/auth-flow-context";
import {
  buildPostAuthDestination,
  sanitizeInternalReturnPath,
} from "@/lib/auth-redirect";
import {
  capturePrivateRouteEvent,
  identifyPrivateRouteUser,
} from "@/lib/private-route-analytics";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style2: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style3: {
    width: "18px",
    height: "18px",
  },
  style4: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style5: {
    textAlign: "center",
  },
  style6: {
    marginBottom: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#8b8174",
  },
  style8: {
    marginTop: "1.5rem",
    textAlign: "center",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: "#8b8174",
  },
  style9: {
    textDecorationLine: "underline",
    textDecorationColor: "#b9ae9f",
    textUnderlineOffset: "2px",
    color: {
      default: null,
      ":hover": "#181613",
    },
  },
  style10: {
    display: "flex",
    flexDirection: "column",
    gap: "1.25rem",
  },
  style11: {
    display: "flex",
    cursor: "pointer",
    alignItems: "center",
    gap: ".25rem",
    alignSelf: "flex-start",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style12: {
    width: ".875rem",
    height: ".875rem",
  },
  style13: {
    display: "flex",
    gap: ".25rem",
    borderRadius: radii.full,
    backgroundColor: "#f4efe6",
    padding: ".25rem",
  },
  style14: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style15: {
    fontWeight: 500,
    color: "#4f4940",
  },
  style16: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style17: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style18: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    gap: ".25rem",
  },
  style19: {
    cursor: "pointer",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty: "color, text-decoration-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    textDecorationLine: {
      default: null,
      ":hover": "underline",
    },
  },
  style20: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty: "color, text-decoration-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    textDecorationLine: {
      default: null,
      ":hover": "underline",
    },
  },
  style21: {
    display: "grid",
    width: "14rem",
    gridTemplateColumns: "18px 1fr",
    alignItems: "center",
    gap: ".75rem",
    textAlign: "left",
  },
  style22: {
    display: {
      default: "flex",
      ":is(*) iconify-icon": "block",
    },
    width: "18px",
    height: "18px",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  modeTab: {
    borderRadius: radii.full,
    cursor: "pointer",
    flexGrow: 1,
    fontSize: ".875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    paddingBlock: ".5rem",
    transitionDuration: ".15s",
    transitionProperty: "background-color, box-shadow, color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
  },
  activeModeTab: {
    backgroundColor: "#fff",
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.05)",
    color: "#181613",
  },
  inactiveModeTab: {
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
  },
});
const commonSearch = {
  redirect: z.string().optional(),
  provider: z.enum(["azure", "github", "google"]).optional(),
  rra: z.boolean().optional(),
};
const validateSearch = flowSearchSchema(commonSearch);
export const Route = createFileRoute("/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
  beforeLoad: async ({ search }) => {
    const user = await fetchUser();
    if (user) {
      const shouldReauthWithProvider =
        search.flow === "web" && !!search.provider;
      if (search.flow === "web" && !shouldReauthWithProvider) {
        throw redirect({
          href: sanitizeInternalReturnPath(search.redirect),
        } as any);
      }
      if (search.flow === "desktop") {
        const result = await createDesktopSession();
        if (result) {
          throw redirect({
            to: "/callback/auth/",
            search: {
              flow: "desktop",
              scheme: search.scheme ?? DEFAULT_DESKTOP_SCHEME,
              access_token: result.access_token,
              refresh_token: result.refresh_token,
            },
          });
        }
      }
    }
    return {
      existingUser: user,
    };
  },
});
type AuthView = "main" | "email" | "sso";
type OAuthProvider = "azure" | "github" | "google";
function getOAuthProviderName(provider: OAuthProvider) {
  return provider === "azure"
    ? "Microsoft"
    : provider.charAt(0).toUpperCase() + provider.slice(1);
}
function Component() {
  const { flow, scheme, redirect, provider, rra } = Route.useSearch();
  const { existingUser } = Route.useRouteContext();
  const [view, setView] = useState<AuthView>("main");
  if (existingUser && flow === "desktop") {
    return (
      <AuthShell
        title="Welcome back"
        description="Finishing your secure handoff to the desktop app."
      >
        <DesktopReauthView
          email={existingUser.email}
          scheme={scheme ?? DEFAULT_DESKTOP_SCHEME}
        />
      </AuthShell>
    );
  }
  if (existingUser && flow === "web" && provider) {
    const providerName = getOAuthProviderName(provider);
    return (
      <AuthShell
        title={`Reconnect ${providerName}`}
        description={`Refresh your ${providerName} access to continue with admin actions.`}
      >
        <div {...stylex.props(styles.style1)}>
          <OAuthButton
            flow={flow}
            scheme={scheme}
            redirect={redirect}
            provider={provider}
            rra={rra}
            autoStart
          />
        </div>
      </AuthShell>
    );
  }
  const showGoogle = !provider || provider === "google";
  const showMicrosoft = !provider || provider === "azure";
  const showGithub = !provider || provider === "github";
  const showEmail = !provider;
  return (
    <AuthShell title="Welcome to Anarlog" showEyebrow={false}>
      {view === "main" && (
        <>
          <div {...stylex.props(styles.style2)}>
            {showGoogle && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="google"
              />
            )}
            {showMicrosoft && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="azure"
              />
            )}
            {showGithub && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="github"
                rra={rra}
              />
            )}
            {showEmail && (
              <button
                onClick={() => setView("email")}
                {...stylex.props(authStyles.secondaryButton)}
              >
                <AuthProviderContent
                  icon={
                    <Envelope
                      {...stylex.props(styles.style3)}
                      aria-hidden="true"
                    />
                  }
                >
                  Sign in with Email
                </AuthProviderContent>
              </button>
            )}
            {showEmail && (
              <button
                onClick={() => setView("sso")}
                {...stylex.props(authStyles.secondaryButton)}
              >
                <AuthProviderContent
                  icon={
                    <Buildings
                      {...stylex.props(styles.style3)}
                      aria-hidden="true"
                    />
                  }
                >
                  Sign in with SSO
                </AuthProviderContent>
              </button>
            )}
          </div>
          <LegalText />
        </>
      )}
      {view === "email" && (
        <EmailAuthView
          flow={flow}
          scheme={scheme}
          redirect={redirect}
          onBack={() => setView("main")}
        />
      )}
      {view === "sso" && (
        <SsoAuthView
          flow={flow}
          scheme={scheme}
          redirect={redirect}
          onBack={() => setView("main")}
        />
      )}
    </AuthShell>
  );
}
function DesktopReauthView({
  email,
  scheme,
}: {
  email: string;
  scheme: DesktopScheme;
}) {
  const retryMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "desktop_reauth",
        flow: "desktop",
      });
      return createDesktopSession();
    },
    onSuccess: (result) => {
      if (result) {
        const params = new URLSearchParams();
        params.set("flow", "desktop");
        params.set("scheme", scheme);
        params.set("access_token", result.access_token);
        params.set("refresh_token", result.refresh_token);
        window.location.href = `/callback/auth?${params.toString()}`;
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "desktop_reauth",
        flow: "desktop",
        failure_stage: "session_handoff",
      });
    },
  });
  useMountEffect(() => {
    retryMutation.mutate();
  });
  const hasRetryFailed =
    retryMutation.isError || (retryMutation.isSuccess && !retryMutation.data);
  return (
    <div {...stylex.props(styles.style1)}>
      {!hasRetryFailed && (
        <div {...stylex.props(authStyles.notice)}>
          <p {...stylex.props(styles.style4)}>Signing in as {email}...</p>
        </div>
      )}
      {hasRetryFailed && (
        <>
          <div {...stylex.props(styles.style5)}>
            <p {...stylex.props(styles.style6)}>Signed in as {email}</p>
            <p {...stylex.props(styles.style7)}>
              Sign in with your provider to continue to the app
            </p>
          </div>
          <div {...stylex.props(styles.style2)}>
            <OAuthButton flow="desktop" scheme={scheme} provider="google" />
            <OAuthButton flow="desktop" scheme={scheme} provider="azure" />
            <OAuthButton flow="desktop" scheme={scheme} provider="github" />
            <SsoAuthView flow="desktop" scheme={scheme} />
          </div>
        </>
      )}
    </div>
  );
}
function LegalText() {
  return (
    <p {...stylex.props(styles.style8)}>
      By signing up, you agree to our{" "}
      <a href="https://anarlog.so/terms" {...stylex.props(styles.style9)}>
        Terms of Service
      </a>{" "}
      and{" "}
      <a href="https://anarlog.so/privacy" {...stylex.props(styles.style9)}>
        Privacy Policy
      </a>
      .
    </p>
  );
}
type EmailMode = "password" | "magic-link";
function EmailAuthView({
  flow,
  scheme,
  redirect,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  onBack: () => void;
}) {
  const [mode, setMode] = useState<EmailMode>("password");
  return (
    <div {...stylex.props(styles.style10)}>
      <button onClick={onBack} {...stylex.props(styles.style11)}>
        <ArrowLeft {...stylex.props(styles.style12)} />
        Back
      </button>

      <div {...stylex.props(styles.style13)}>
        <button
          onClick={() => setMode("password")}
          {...stylex.props([
            styles.modeTab,
            mode === "password" ? styles.activeModeTab : styles.inactiveModeTab,
          ])}
        >
          Password
        </button>
        <button
          onClick={() => setMode("magic-link")}
          {...stylex.props([
            styles.modeTab,
            mode === "magic-link"
              ? styles.activeModeTab
              : styles.inactiveModeTab,
          ])}
        >
          Magic link
        </button>
      </div>

      {mode === "password" && (
        <PasswordForm flow={flow} scheme={scheme} redirect={redirect} />
      )}
      {mode === "magic-link" && (
        <MagicLinkForm flow={flow} scheme={scheme} redirect={redirect} />
      )}

      <LegalText />
    </div>
  );
}
function SsoAuthView({
  flow,
  scheme,
  redirect,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  onBack?: () => void;
}) {
  const [domain, setDomain] = useState("");
  const ssoMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "sso",
        flow,
      });
      return doSsoAuth({
        data: {
          domain,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
        return;
      }
      capturePrivateRouteEvent("auth_failed", {
        method: "sso",
        flow,
        failure_stage: "provider",
      });
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "sso",
        flow,
        failure_stage: "request",
      });
    },
  });
  return (
    <div {...stylex.props(styles.style10)}>
      {onBack ? (
        <button onClick={onBack} {...stylex.props(styles.style11)}>
          <ArrowLeft {...stylex.props(styles.style12)} />
          Back
        </button>
      ) : null}
      <form
        {...stylex.props(styles.style2)}
        onSubmit={(event) => {
          event.preventDefault();
          if (domain.trim()) ssoMutation.mutate();
        }}
      >
        <input
          type="text"
          autoComplete="organization"
          placeholder="you@company.com"
          value={domain}
          onChange={(event) => setDomain(event.target.value)}
          {...stylex.props(authStyles.input)}
        />
        <button
          type="submit"
          disabled={!domain.trim() || ssoMutation.isPending}
          {...stylex.props(authStyles.primaryButton)}
        >
          Continue with SSO
        </button>
        {ssoMutation.data &&
        "error" in ssoMutation.data &&
        ssoMutation.data.error ? (
          <p {...stylex.props(styles.style14)}>{ssoMutation.data.message}</p>
        ) : null}
      </form>
      {onBack ? <LegalText /> : null}
    </div>
  );
}
function PasswordForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isSignUp, setIsSignUp] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const signInMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "password",
        action: "sign_in",
        flow,
      });
      return doPasswordSignIn({
        data: {
          email,
          password,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_in",
          flow,
          failure_stage: "provider",
        });
        setErrorMessage(
          (
            result as {
              error: boolean;
              message: string;
            }
          ).message,
        );
        return;
      }
      if (
        result &&
        "success" in result &&
        result.success &&
        "access_token" in result
      ) {
        identifyPrivateRouteUser(
          "userId" in result
            ? (result.userId as string | undefined)
            : undefined,
          {
            method: "password",
            action: "sign_in",
            flow,
          },
        );
        handlePasswordSuccess(
          result.access_token as string,
          result.refresh_token as string,
          flow,
          scheme,
          redirect,
          false,
        );
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "password",
        action: "sign_in",
        flow,
        failure_stage: "request",
      });
    },
  });
  const signUpMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "password",
        action: "sign_up",
        flow,
      });
      return doPasswordSignUp({
        data: {
          name,
          email,
          password,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "provider",
        });
        setErrorMessage(
          (
            result as {
              error: boolean;
              message: string;
            }
          ).message,
        );
        return;
      }
      if (result && "success" in result && result.success) {
        identifyPrivateRouteUser(
          "userId" in result
            ? (result.userId as string | undefined)
            : undefined,
          {
            method: "password",
            action: "sign_up",
            flow,
          },
        );
        if ("needsConfirmation" in result && result.needsConfirmation) {
          setSubmitted(true);
          return;
        }
        if ("access_token" in result) {
          handlePasswordSuccess(
            result.access_token as string,
            result.refresh_token as string,
            flow,
            scheme,
            redirect,
            "newAccount" in result && result.newAccount,
          );
        }
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "password",
        action: "sign_up",
        flow,
        failure_stage: "request",
      });
    },
  });
  const isPending = signInMutation.isPending || signUpMutation.isPending;
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");
    if (isSignUp) {
      if (password !== confirmPassword) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "validation",
        });
        setErrorMessage("Passwords do not match");
        return;
      }
      if (password.length < 6) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "validation",
        });
        setErrorMessage("Password must be at least 6 characters");
        return;
      }
      signUpMutation.mutate();
    } else {
      signInMutation.mutate();
    }
  };
  if (submitted) {
    return (
      <div {...stylex.props(authStyles.notice)}>
        <p {...stylex.props(styles.style15)}>Check your email</p>
        <p {...stylex.props(styles.style16)}>
          We sent a confirmation link to {email}
        </p>
      </div>
    );
  }
  return (
    <form onSubmit={handleSubmit} {...stylex.props(styles.style2)}>
      {isSignUp && (
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Name"
          autoComplete="name"
          required
          {...stylex.props(authStyles.input)}
        />
      )}
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
        required
        {...stylex.props(authStyles.input)}
      />
      <input
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
        required
        {...stylex.props(authStyles.input)}
      />
      {isSignUp && (
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm password"
          required
          {...stylex.props(authStyles.input)}
        />
      )}
      {errorMessage && <p {...stylex.props(styles.style17)}>{errorMessage}</p>}
      <button
        type="submit"
        disabled={
          isPending ||
          !email ||
          !password ||
          (isSignUp && (!name.trim() || !confirmPassword))
        }
        {...stylex.props(authStyles.primaryButton)}
      >
        {isPending ? "Loading..." : isSignUp ? "Create account" : "Sign in"}
      </button>
      <div {...stylex.props(styles.style18)}>
        <button
          type="button"
          onClick={() => {
            setIsSignUp(!isSignUp);
            setErrorMessage("");
            setName("");
            setConfirmPassword("");
          }}
          {...stylex.props(styles.style19)}
        >
          {isSignUp
            ? "Already have an account? Sign in"
            : "Don't have an account? Sign up"}
        </button>
        {!isSignUp && (
          <Link
            to="/reset-password/"
            search={toAuthFlowSearch({
              flow,
              scheme,
              redirect,
            })}
            {...stylex.props(styles.style20)}
          >
            Forgot password?
          </Link>
        )}
      </div>
    </form>
  );
}
function handlePasswordSuccess(
  accessToken: string,
  refreshToken: string,
  flow: "desktop" | "web",
  scheme?: DesktopScheme,
  redirectPath?: string,
  newAccount = false,
) {
  if (flow === "desktop") {
    const params = new URLSearchParams();
    params.set("flow", "desktop");
    if (scheme) params.set("scheme", scheme);
    params.set("access_token", accessToken);
    params.set("refresh_token", refreshToken);
    window.location.href = `/callback/auth?${params.toString()}`;
  } else {
    window.location.href = buildPostAuthDestination({
      newAccount,
      returnTo: redirectPath,
    });
  }
}
function MagicLinkForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const magicLinkMutation = useMutation({
    mutationFn: (email: string) => {
      capturePrivateRouteEvent("auth_started", {
        method: "magic_link",
        flow,
      });
      return doMagicLinkAuth({
        data: {
          email,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result && !("error" in result)) {
        setSubmitted(true);
      } else {
        capturePrivateRouteEvent("auth_failed", {
          method: "magic_link",
          flow,
          failure_stage: "provider",
        });
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "magic_link",
        flow,
        failure_stage: "request",
      });
    },
  });
  if (submitted) {
    return (
      <div {...stylex.props(authStyles.notice)}>
        <p {...stylex.props(styles.style15)}>Check your email</p>
        <p {...stylex.props(styles.style16)}>We sent a magic link to {email}</p>
      </div>
    );
  }
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (email) {
          magicLinkMutation.mutate(email);
        }
      }}
      {...stylex.props(styles.style2)}
    >
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Enter your email"
        required
        {...stylex.props(authStyles.input)}
      />
      <button
        type="submit"
        disabled={magicLinkMutation.isPending || !email}
        {...stylex.props(authStyles.primaryButton)}
      >
        {magicLinkMutation.isPending ? "Sending..." : "Send magic link"}
      </button>
      {magicLinkMutation.isError && (
        <p {...stylex.props(styles.style17)}>
          Failed to send magic link. Please try again.
        </p>
      )}
    </form>
  );
}
function AuthProviderContent({
  icon,
  children,
}: {
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <span {...stylex.props(styles.style21)}>
      <span {...stylex.props(styles.style22)}>{icon}</span>
      <span>{children}</span>
    </span>
  );
}
function OAuthButton({
  flow,
  scheme,
  redirect,
  provider,
  rra,
  autoStart = false,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  provider: OAuthProvider;
  rra?: boolean;
  autoStart?: boolean;
}) {
  const oauthMutation = useMutation({
    mutationFn: (provider: OAuthProvider) => {
      capturePrivateRouteEvent("auth_started", {
        method: "oauth",
        provider,
        flow,
      });
      return doAuth({
        data: {
          provider,
          flow,
          scheme,
          redirect,
          rra,
        },
      });
    },
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
      } else {
        capturePrivateRouteEvent("auth_failed", {
          method: "oauth",
          provider,
          flow,
          failure_stage: "provider",
        });
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "oauth",
        provider,
        flow,
        failure_stage: "request",
      });
    },
  });
  const { mutate, isPending } = oauthMutation;
  const hasAutoStartedRef = useRef(false);
  useMountEffect(() => {
    if (autoStart && !hasAutoStartedRef.current) {
      hasAutoStartedRef.current = true;
      mutate(provider);
    }
  });
  return (
    <button
      onClick={() => mutate(provider)}
      disabled={isPending}
      {...stylex.props(authStyles.secondaryButton)}
    >
      <AuthProviderContent
        icon={
          provider === "google" ? (
            <Icon icon="logos:google-icon" width="18" height="18" />
          ) : provider === "github" ? (
            <Icon icon="logos:github-icon" width="18" height="18" />
          ) : (
            <Icon icon="logos:microsoft-icon" width="18" height="18" />
          )
        }
      >
        Sign in with {getOAuthProviderName(provider)}
      </AuthProviderContent>
    </button>
  );
}
