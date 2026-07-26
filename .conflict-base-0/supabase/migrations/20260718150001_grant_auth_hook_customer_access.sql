GRANT SELECT (id, invoice_settings, default_source)
ON TABLE stripe.customers
TO supabase_auth_admin;
