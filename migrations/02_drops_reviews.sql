CREATE TABLE IF NOT EXISTS drops_reviews (
    drop_id INTEGER REFERENCES drops (id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (drop_id, user_id)
)
STRICT;
