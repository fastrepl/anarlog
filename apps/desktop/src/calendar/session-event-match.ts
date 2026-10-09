export const SESSION_EVENT_MATCH = `event.deleted_at IS NULL
        AND (
          event.id = session.event_id
          OR (
            event.tracking_id_event <> ''
            AND event.tracking_id_event = CASE
              WHEN json_valid(session.event_json)
              THEN json_extract(session.event_json, '$.tracking_id')
              ELSE ''
            END
            AND event.calendar_id = CASE
              WHEN json_valid(session.event_json)
              THEN json_extract(session.event_json, '$.calendar_id')
              ELSE ''
            END
          )
        )`;
export const SESSION_EVENT_ORDER = `CASE WHEN event.id = session.event_id THEN 0 ELSE 1 END,
  event.started_at, event.id`;
