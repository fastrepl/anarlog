CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  matched_customer_id text;
BEGIN
  SELECT customer.id
  INTO matched_customer_id
  FROM stripe.customers AS customer
  WHERE NEW.email IS NOT NULL
    AND customer.email IS NOT NULL
    AND pg_catalog.lower(pg_catalog.btrim(customer.email))
      = pg_catalog.lower(pg_catalog.btrim(NEW.email))
    AND NOT EXISTS (
      SELECT 1
      FROM public.profiles AS linked_profile
      WHERE linked_profile.stripe_customer_id = customer.id
    )
  ORDER BY
    CASE
      WHEN EXISTS (
        SELECT 1
        FROM stripe.subscriptions AS subscription
        WHERE subscription.customer = customer.id
          AND subscription.status IN ('active', 'past_due', 'unpaid')
      ) THEN 0
      WHEN EXISTS (
        SELECT 1
        FROM stripe.subscriptions AS subscription
        WHERE subscription.customer = customer.id
          AND subscription.status = 'trialing'
      ) THEN 1
      WHEN EXISTS (
        SELECT 1
        FROM stripe.subscriptions AS subscription
        WHERE subscription.customer = customer.id
      ) THEN 2
      ELSE 3
    END,
    customer.created ASC NULLS LAST,
    customer.id
  LIMIT 1;

  INSERT INTO public.profiles (id, stripe_customer_id)
  VALUES (NEW.id, matched_customer_id);

  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION public.handle_user_email_update()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  matched_customer_id text;
BEGIN
  IF OLD.email IS DISTINCT FROM NEW.email THEN
    SELECT profile.stripe_customer_id
    INTO matched_customer_id
    FROM public.profiles AS profile
    WHERE profile.id = NEW.id;

    IF matched_customer_id IS NULL THEN
      SELECT customer.id
      INTO matched_customer_id
      FROM stripe.customers AS customer
      WHERE NEW.email IS NOT NULL
        AND customer.email IS NOT NULL
        AND pg_catalog.lower(pg_catalog.btrim(customer.email))
          = pg_catalog.lower(pg_catalog.btrim(NEW.email))
        AND NOT EXISTS (
          SELECT 1
          FROM public.profiles AS linked_profile
          WHERE linked_profile.stripe_customer_id = customer.id
        )
      ORDER BY
        CASE
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
              AND subscription.status IN ('active', 'past_due', 'unpaid')
          ) THEN 0
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
              AND subscription.status = 'trialing'
          ) THEN 1
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
          ) THEN 2
          ELSE 3
        END,
        customer.created ASC NULLS LAST,
        customer.id
      LIMIT 1;

      IF matched_customer_id IS NOT NULL THEN
        UPDATE public.profiles AS profile
        SET stripe_customer_id = matched_customer_id
        WHERE profile.id = NEW.id;
      END IF;
    END IF;
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION public.handle_new_user()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.handle_new_user()
  TO supabase_auth_admin;

REVOKE ALL ON FUNCTION public.handle_user_email_update()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.handle_user_email_update()
  TO supabase_auth_admin;

WITH ranked_candidates AS (
  SELECT
    profile.id AS profile_id,
    customer.id AS customer_id,
    pg_catalog.row_number() OVER (
      PARTITION BY profile.id
      ORDER BY
        CASE
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
              AND subscription.status IN ('active', 'past_due', 'unpaid')
          ) THEN 0
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
              AND subscription.status = 'trialing'
          ) THEN 1
          WHEN EXISTS (
            SELECT 1
            FROM stripe.subscriptions AS subscription
            WHERE subscription.customer = customer.id
          ) THEN 2
          ELSE 3
        END,
        customer.created ASC NULLS LAST,
        customer.id
    ) AS rank
  FROM public.profiles AS profile
  JOIN auth.users AS account
    ON account.id = profile.id
  JOIN stripe.customers AS customer
    ON customer.email IS NOT NULL
    AND account.email IS NOT NULL
    AND pg_catalog.lower(pg_catalog.btrim(customer.email))
      = pg_catalog.lower(pg_catalog.btrim(account.email))
  WHERE profile.stripe_customer_id IS NULL
    AND NOT EXISTS (
      SELECT 1
      FROM public.profiles AS linked_profile
      WHERE linked_profile.stripe_customer_id = customer.id
    )
)
UPDATE public.profiles AS profile
SET stripe_customer_id = candidate.customer_id
FROM ranked_candidates AS candidate
WHERE candidate.profile_id = profile.id
  AND candidate.rank = 1
  AND profile.stripe_customer_id IS NULL;
