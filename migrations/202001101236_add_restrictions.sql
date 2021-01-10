CREATE TABLE restrictions (
    user_id TEXT PRIMARY KEY,
    restrictor_user_id TEXT NOT NULL,
    time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);