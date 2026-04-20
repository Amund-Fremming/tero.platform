CREATE TABLE beer_tracker_games (
    id TEXT PRIMARY KEY,
    can_size DOUBLE PRECISION NOT NULL,
    goal INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE beer_tracker_members (
    game_id TEXT NOT NULL REFERENCES beer_tracker_games(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (game_id, name)
);

CREATE INDEX idx_beer_tracker_members_game_id ON beer_tracker_members(game_id);
