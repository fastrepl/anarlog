import { PRO_TRIAL_DAYS } from "@hypr/pricing/trial";

export const WEB_TRIAL_CHECKOUT_FIELDS = {
  payment_method_collection: "always",
  subscription_data: {
    trial_period_days: PRO_TRIAL_DAYS,
    trial_settings: {
      end_behavior: {
        missing_payment_method: "cancel",
      },
    },
  },
} as const;
