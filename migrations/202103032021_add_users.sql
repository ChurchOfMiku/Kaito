CREATE TABLE users (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    role TEXT,
    discord_id BLOB(8) UNIQUE, -- 8 bytes / 64 bits
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Start at uid 1101
UPDATE sqlite_sequence SET seq = 1100 WHERE name = 'users';
