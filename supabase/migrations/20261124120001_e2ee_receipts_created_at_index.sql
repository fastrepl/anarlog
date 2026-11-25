CREATE INDEX CONCURRENTLY IF NOT EXISTS e2ee_replica_receipts_created_at_idx
  ON public.e2ee_replica_receipts (created_at);
