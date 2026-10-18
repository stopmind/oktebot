CREATE TABLE itch_index_subs (
    user_id INT REFERENCES users (id) ON DELETE CASCADE NOT NULL,
    game_id INT NOT NULL,
    PRIMARY KEY (user_id, game_id)
)
WITHOUT ROWID, STRICT;
