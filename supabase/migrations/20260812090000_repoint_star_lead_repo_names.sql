-- Star-lead sync now records repo_name as 'fastrepl/anarlog'. Event dedupe keys
-- on (github_username, event_type, repo_name, event_at), so historical rows left
-- on the pre-rename names would make the next full sync reinsert every past
-- event.

UPDATE public.github_star_leads
SET repo_name = 'fastrepl/anarlog'
WHERE repo_name IN ('fastrepl/char', 'fastrepl/hyprnote');

ALTER TABLE public.github_star_leads
  ALTER COLUMN repo_name SET DEFAULT 'fastrepl/anarlog';

-- github_star_lead_events is provisioned outside this migration set, so skip it
-- on databases that do not have it yet.
DO $$
BEGIN
  IF to_regclass('public.github_star_lead_events') IS NULL THEN
    RETURN;
  END IF;

  -- Post-rename syncs may already have written the new name for an event that
  -- also exists under an old one; drop the stale duplicate before renaming.
  DELETE FROM public.github_star_lead_events AS legacy
  WHERE legacy.repo_name IN ('fastrepl/char', 'fastrepl/hyprnote')
    AND EXISTS (
      SELECT 1
      FROM public.github_star_lead_events AS renamed
      WHERE renamed.repo_name = 'fastrepl/anarlog'
        AND renamed.github_username = legacy.github_username
        AND renamed.event_type = legacy.event_type
        AND renamed.event_at = legacy.event_at
    );

  UPDATE public.github_star_lead_events
  SET repo_name = 'fastrepl/anarlog'
  WHERE repo_name IN ('fastrepl/char', 'fastrepl/hyprnote');
END
$$;
