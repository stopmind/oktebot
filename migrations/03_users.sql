CREATE TABLE IF NOT EXISTS users (
    id         INTEGER PRIMARY KEY
                       NOT NULL
                       UNIQUE,
    username   TEXT    NOT NULL,
    reputation INTEGER NOT NULL
                       DEFAULT (0),
    bio        TEXT
)
WITHOUT ROWID,
STRICT;
