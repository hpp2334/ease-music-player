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
    picture BLOB
);

CREATE TABLE IF NOT EXISTS music (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    path TEXT NOT NULL,
    storage_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    duration BIGINT,
    picture BLOB,
    lyric_storage_id INTEGER,
    lyric_path TEXT,
    picture_storage_id INTEGER,
    picture_path TEXT,
    UNIQUE(storage_id, path)
);


CREATE TABLE IF NOT EXISTS playlist_music (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    playlist_id INTEGER NOT NULL,
    music_id INTEGER NOT NULL,
    UNIQUE(playlist_id, music_id)
);
