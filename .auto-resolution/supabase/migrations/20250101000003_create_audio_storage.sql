INSERT INTO storage.buckets (id, name, public)
VALUES ('audio-files', 'audio-files', false)
ON CONFLICT (id) DO NOTHING;

CREATE POLICY "audio_files_select_owner" ON storage.objects FOR SELECT TO authenticated USING (bucket_id = 'audio-files' AND (SELECT auth.uid())::text = (storage.foldername(name))[1]);

CREATE POLICY "audio_files_insert_authenticated" ON storage.objects FOR INSERT TO authenticated WITH CHECK (bucket_id = 'audio-files' AND (SELECT auth.uid())::text = (storage.foldername(name))[1]);

CREATE POLICY "audio_files_update_owner" ON storage.objects FOR UPDATE TO authenticated USING (bucket_id = 'audio-files' AND (SELECT auth.uid())::text = (storage.foldername(name))[1]);

CREATE POLICY "audio_files_delete_owner" ON storage.objects FOR DELETE TO authenticated USING (bucket_id = 'audio-files' AND (SELECT auth.uid())::text = (storage.foldername(name))[1]);
