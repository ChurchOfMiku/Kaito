CREATE TABLE servers (
    sid INTEGER PRIMARY KEY AUTOINCREMENT,
    discord_id BLOB(8) UNIQUE, -- 8 bytes / 64 bits
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tags (
    key TEXT NOT NULL,
    sid INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    transfer_uid INTEGER,
    value TEXT NOT NULL,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    edit_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(uid) REFERENCES users(uid),
    FOREIGN KEY(sid) REFERENCES servers(sid),
    PRIMARY KEY (key, sid)
);
