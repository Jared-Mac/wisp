ALTER TABLE spots ADD COLUMN category_id TEXT REFERENCES channel_categories(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_spots_category_position
ON spots(category_id, name COLLATE NOCASE);
