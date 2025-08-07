package com.kutedev.easemusicplayer.core

import uniffi.ease_client_backend.DataSourceKey


class DataSourceKeyH(key: DataSourceKey) {
    private val _key = key;

    fun value(): DataSourceKey {
        return this._key
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DataSourceKeyH) return false

        if (this._key is DataSourceKey.Music && other._key is DataSourceKey.Music) {
            return this._key.id == other._key.id
        }
        if (this._key is DataSourceKey.Cover && other._key is DataSourceKey.Cover) {
            return this._key.id == other._key.id
        }
        if (this._key is DataSourceKey.AnyEntry && other._key is DataSourceKey.AnyEntry) {
            return this._key.entry.storageId == other._key.entry.storageId && this._key.entry.path == other._key.entry.path;
        }
        return false;
    }

    override fun hashCode(): Int {
        return _key.hashCode()
    }
}
