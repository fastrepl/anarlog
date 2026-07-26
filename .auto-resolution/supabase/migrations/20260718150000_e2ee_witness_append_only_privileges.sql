REVOKE ALL ON TABLE public.e2ee_freshness_events FROM service_role;
GRANT SELECT, INSERT ON TABLE public.e2ee_freshness_events TO service_role;

REVOKE ALL ON SEQUENCE public.e2ee_freshness_events_sequence_seq
  FROM service_role;
GRANT USAGE, SELECT ON SEQUENCE public.e2ee_freshness_events_sequence_seq
  TO service_role;
