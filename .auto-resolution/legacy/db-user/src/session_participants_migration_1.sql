ALTER TABLE
  session_participants
ADD
  COLUMN deleted BOOLEAN DEFAULT FALSE;
