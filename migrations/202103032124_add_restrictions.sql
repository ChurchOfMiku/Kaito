CREATE TABLE restrictions (
    uid INTEGER PRIMARY KEY,
    restrictor_user_id INTEGER NOT NULL,
    time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(uid) REFERENCES users(uid)
);