REVOKE EXECUTE ON FUNCTION
  public.read_my_session_share_snapshot(uuid),
  public.read_my_session_share_snapshot_v2(uuid),
  public.read_my_session_share_snapshot_with_attachments(uuid),
  public.list_my_session_share_snapshots(),
  public.list_my_session_share_snapshots_with_attachments(),
  public.list_my_session_share_snapshot_page(uuid, integer),
  public.list_my_session_share_snapshot_page_v2(uuid, integer),
  public.list_my_session_share_snapshot_page_with_attachments(uuid, integer),
  public.read_session_share_link_snapshot(uuid, text),
  public.read_public_session_share_snapshot(text)
FROM service_role;
