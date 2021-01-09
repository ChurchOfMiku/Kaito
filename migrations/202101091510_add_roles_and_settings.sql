CREATE TABLE roles (
    user_id TEXT PRIMARY KEY,
    role TEXT NOT NULL
);

CREATE TABLE settings_server (
    server_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (server_id, key)
);

CREATE TABLE settings_channel (
    channel_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (channel_id, key)
);
