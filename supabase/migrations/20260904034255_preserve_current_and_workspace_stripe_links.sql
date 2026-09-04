CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  matched_customer_id text;
BEGIN
  IF NEW.email IS NOT NULL THEN
    PERFORM pg_catalog.pg_advisory_xact_lock(
      pg_catalog.hashtextextended(
        pg_catalog.lower(pg_catalog.btrim(NEW.email)),
        4030317
      )
    );
  END IF;

  SELECT customer.id
  INTO matched_customer_id
  FROM stripe.customers AS customer
  WHERE NEW.email IS NOT NULL
    AND customer.email IS NOT NULL
    AND NOT customer.deleted
    AND pg_catalog.lower(pg_catalog.btrim(customer.email))
      = pg_catalog.lower(pg_catalog.btrim(NEW.email))
    AND NOT EXISTS (
      SELECT 1
      FROM public.profiles AS linked_profile
      WHERE linked_profile.stripe_customer_id = customer.id
    )
    AND NOT EXISTS (
      SELECT 1
      FROM public.workspaces AS linked_workspace
      WHERE linked_workspace.stripe_customer_id = customer.id
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
          AND subscription.status IN ('trialing', 'paused')
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
    IF NEW.email IS NOT NULL THEN
      PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
          pg_catalog.lower(pg_catalog.btrim(NEW.email)),
          4030317
        )
      );
    END IF;

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
        AND NOT customer.deleted
        AND pg_catalog.lower(pg_catalog.btrim(customer.email))
          = pg_catalog.lower(pg_catalog.btrim(NEW.email))
        AND NOT EXISTS (
          SELECT 1
          FROM public.profiles AS linked_profile
          WHERE linked_profile.stripe_customer_id = customer.id
        )
        AND NOT EXISTS (
          SELECT 1
          FROM public.workspaces AS linked_workspace
          WHERE linked_workspace.stripe_customer_id = customer.id
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
              AND subscription.status IN ('trialing', 'paused')
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
