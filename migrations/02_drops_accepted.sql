CREATE TABLE IF NOT EXISTS drops_accepted (
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    drop_id INTEGER REFERENCES drops (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, drop_id)
)
STRICT;
