CREATE TABLE IF NOT EXISTS users_roles (
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE
                    NOT NULL,
    role    INTEGER NOT NULL,
    PRIMARY KEY (user_id, role)
)
WITHOUT ROWID,
STRICT;
