CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger AS $$
DECLARE
  matched_customer_id text;
BEGIN
  SELECT id INTO matched_customer_id
  FROM stripe.customers
  WHERE email = NEW.email
  LIMIT 1;

  INSERT INTO public.profiles (id, stripe_customer_id)
  VALUES (NEW.id, matched_customer_id);

  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE OR REPLACE FUNCTION public.handle_user_email_update()
RETURNS trigger AS $$
DECLARE
  matched_customer_id text;
BEGIN
  IF OLD.email IS DISTINCT FROM NEW.email THEN
    SELECT stripe_customer_id INTO matched_customer_id
    FROM public.profiles
    WHERE id = NEW.id;

    IF matched_customer_id IS NULL THEN
      SELECT id INTO matched_customer_id
      FROM stripe.customers
      WHERE email = NEW.email
      LIMIT 1;

      IF matched_customer_id IS NOT NULL THEN
        UPDATE public.profiles
        SET stripe_customer_id = matched_customer_id
        WHERE id = NEW.id;
      END IF;
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP TRIGGER IF EXISTS on_auth_user_email_updated ON auth.users;

CREATE TRIGGER on_auth_user_email_updated
  AFTER UPDATE ON auth.users
  FOR EACH ROW EXECUTE FUNCTION public.handle_user_email_update();
