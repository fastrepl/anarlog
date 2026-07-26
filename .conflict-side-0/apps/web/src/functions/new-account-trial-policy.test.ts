import assert from "node:assert/strict";
import test from "node:test";

import {
  isConfirmedNewAccount,
  type NewAccountAuthMethod,
  shouldOfferNewAccountTrialCheckout,
} from "./new-account-trial-policy.ts";

const confirmedAt = "2026-07-17T06:00:00.000Z";

function user(
  overrides: Partial<{
    created_at: string;
    confirmed_at: string;
    email_confirmed_at: string;
    phone_confirmed_at: string;
    last_sign_in_at: string;
  }> = {},
) {
  return {
    created_at: "2026-07-17T05:50:00.000Z",
    confirmed_at: confirmedAt,
    email_confirmed_at: confirmedAt,
    phone_confirmed_at: undefined,
    last_sign_in_at: confirmedAt,
    ...overrides,
  };
}

for (const method of ["password-signup", "signup", "invite"] as const) {
  test(`${method} is an explicit new-account confirmation`, () => {
    assert.equal(
      isConfirmedNewAccount(
        user({ last_sign_in_at: "2026-08-17T06:00:00.000Z" }),
        method,
      ),
      true,
    );
  });
}

for (const method of ["oauth", "email", "magiclink"] as const) {
  test(`${method} qualifies for checkout on the initial confirmed session`, () => {
    assert.equal(isConfirmedNewAccount(user(), method), true);
  });

  test(`${method} does not qualify a returning user`, () => {
    assert.equal(
      isConfirmedNewAccount(
        user({ last_sign_in_at: "2026-08-17T06:00:00.000Z" }),
        method,
      ),
      false,
    );
  });
}

test("a delayed first magic-link confirmation still qualifies", () => {
  assert.equal(
    isConfirmedNewAccount(
      user({ created_at: "2026-07-16T06:00:00.000Z" }),
      "magiclink",
    ),
    true,
  );
});

for (const method of ["recovery", "email_change"] as NewAccountAuthMethod[]) {
  test(`${method} never offers a new-account trial`, () => {
    assert.equal(isConfirmedNewAccount(user(), method), false);
  });
}

test("missing or invalid sign-in timestamps do not qualify", () => {
  assert.equal(
    isConfirmedNewAccount(user({ last_sign_in_at: undefined }), "oauth"),
    false,
  );
  assert.equal(
    isConfirmedNewAccount(user({ last_sign_in_at: "invalid" }), "magiclink"),
    false,
  );
});

test("only web auth offers the card-required trial checkout", () => {
  assert.equal(
    shouldOfferNewAccountTrialCheckout({
      flow: "web",
      method: "oauth",
      user: user(),
    }),
    true,
  );
  assert.equal(
    shouldOfferNewAccountTrialCheckout({
      flow: "desktop",
      method: "oauth",
      user: user(),
    }),
    false,
  );
});
