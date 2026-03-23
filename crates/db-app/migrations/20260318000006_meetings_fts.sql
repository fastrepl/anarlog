CREATE VIRTUAL TABLE IF NOT EXISTS meetings_fts USING fts5(title, summary, memo, content=meetings, content_rowid=rowid);
CREATE TRIGGER IF NOT EXISTS meetings_ai AFTER INSERT ON meetings BEGIN
  INSERT INTO meetings_fts(rowid, title, summary, memo) VALUES (new.rowid, new.title, new.summary, new.memo);
END;
CREATE TRIGGER IF NOT EXISTS meetings_ad AFTER DELETE ON meetings BEGIN
  INSERT INTO meetings_fts(meetings_fts, rowid, title, summary, memo) VALUES ('delete', old.rowid, old.title, old.summary, old.memo);
END;
CREATE TRIGGER IF NOT EXISTS meetings_au AFTER UPDATE ON meetings BEGIN
  INSERT INTO meetings_fts(meetings_fts, rowid, title, summary, memo) VALUES ('delete', old.rowid, old.title, old.summary, old.memo);
  INSERT INTO meetings_fts(rowid, title, summary, memo) VALUES (new.rowid, new.title, new.summary, new.memo);
END;
