CREATE TABLE IF NOT EXISTS storage (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    addr TEXT NOT NULL,
    alias TEXT,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    is_anonymous BOOLEAN NOT NULL,
    typ INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS playlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    title TEXT NOT NULL,
    created_time INTEGER NOT NULL,
    picture_storage_id INTEGER,
    picture_path TEXT
);

CREATE TABLE IF NOT EXISTS music (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    path TEXT NOT NULL,
    storage_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    duration BIGINT,
    lyric_storage_id INTEGER,
    lyric_path TEXT,
    lyric_default BOOLEAN NOT NULL,
    cover BLOB,
    UNIQUE(storage_id, path)
);


CREATE TABLE IF NOT EXISTS playlist_music (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    playlist_id INTEGER NOT NULL,
    music_id INTEGER NOT NULL,
    UNIQUE(playlist_id, music_id)
);
