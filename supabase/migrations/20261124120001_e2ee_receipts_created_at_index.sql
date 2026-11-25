-- Standalone migration outside 20261124120000_e2ee_sync_efficiency.sql; Supabase CLI runs
-- migrations transactionally (so CONCURRENTLY is rejected) and receipts are pruned to 7 days.
CREATE INDEX IF NOT EXISTS e2ee_replica_receipts_created_at_idx
  ON public.e2ee_replica_receipts (created_at);
