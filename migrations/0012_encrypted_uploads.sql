ALTER TABLE file_uploads ADD COLUMN encryption_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE file_uploads ADD COLUMN encrypted_message_id TEXT;
ALTER TABLE file_uploads ADD COLUMN roster_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE file_uploads ADD COLUMN plaintext_size INTEGER;
