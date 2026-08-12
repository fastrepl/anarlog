-- The GitHub star-lead pipeline (sync, LLM research, admin endpoints) is gone,
-- so nothing reads or writes these tables. github_star_lead_events was
-- provisioned outside this migration set, hence the IF EXISTS.

DROP TABLE IF EXISTS public.github_star_lead_events;
DROP TABLE IF EXISTS public.github_star_leads;
