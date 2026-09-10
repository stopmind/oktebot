CREATE TABLE users_new (
    id         INTEGER PRIMARY KEY NOT NULL UNIQUE,
    username   TEXT,
    firstname  TEXT    NOT NULL,
    reputation INTEGER NOT NULL DEFAULT (0),
    bio        TEXT,
    banned     INTEGER NOT NULL DEFAULT (0)
)
WITHOUT ROWID, STRICT;

INSERT INTO users_new (id, username, firstname, reputation, bio, banned)
SELECT id, username, '', reputation, bio, banned
FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;
