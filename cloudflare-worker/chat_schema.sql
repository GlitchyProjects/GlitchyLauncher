CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    sender_username TEXT NOT NULL,
    sender_badge TEXT,
    sender_model TEXT DEFAULT 'default',
    sender_avatar TEXT,
    text TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_channel_ts ON chat_messages(channel_id, timestamp);

CREATE TABLE IF NOT EXISTS chat_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    owner_id TEXT NOT NULL,
    invite_code TEXT UNIQUE NOT NULL,
    visibility TEXT NOT NULL,
    members_count INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_groups_owner ON chat_groups(owner_id);
