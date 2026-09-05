package com.kutedev.easemusicplayer.utils

import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns

/**
 * Helpers for resolving native file picker results (`ACTION_OPEN_DOCUMENT`)
 * back to real file paths that the Rust `LocalBackend` can read
 * (`/storage/emulated/0` + path).
 */
object SafUri {
    /** Mirror of `LocalBackend::ANDROID_PREFIX_PATH` (ease-remote-storage). */
    private const val LOCAL_PREFIX = "/storage/emulated/0"

    private const val MEDIA_AUTHORITY = "media"
    private const val EXTERNAL_STORAGE_AUTHORITY = "com.android.externalstorage.documents"
    private const val PRIMARY_VOLUME_DOC_ID_PREFIX = "primary:"
    private const val DATA_COLUMN = "_data"

    /**
     * Resolve a picked [uri] to a `LocalBackend`-compatible path (e.g. "/Music/a.mp3").
     *
     * Returns null when the file does not live on the primary external storage
     * (cloud providers, removable volumes, ...) — such picks cannot be read by
     * the current local backend and must be skipped by callers.
     */
    fun resolveLocalPath(context: Context, uri: Uri): String? {
        val absolute = when (uri.scheme) {
            "file" -> uri.path
            "content" -> resolveContentPath(context, uri)
            else -> null
        } ?: return null

        if (!absolute.startsWith("$LOCAL_PREFIX/")) {
            return null
        }
        return absolute.removePrefix(LOCAL_PREFIX)
    }

    /** `OpenableColumns.DISPLAY_NAME` of the picked file, or a best-effort fallback. */
    fun queryDisplayName(context: Context, uri: Uri): String? {
        val cursor = queryColumns(context, uri, arrayOf(OpenableColumns.DISPLAY_NAME)) ?: run {
            return uri.lastPathSegment?.substringAfterLast('/')
        }
        cursor.use {
            if (it.moveToFirst()) {
                val index = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (index >= 0 && !it.isNull(index)) {
                    return it.getString(index)
                }
            }
        }
        return uri.lastPathSegment?.substringAfterLast('/')
    }

    /** `OpenableColumns.SIZE` of the picked file, or null when unavailable. */
    fun querySize(context: Context, uri: Uri): ULong? {
        val cursor = queryColumns(context, uri, arrayOf(OpenableColumns.SIZE)) ?: return null
        cursor.use {
            if (it.moveToFirst()) {
                val index = it.getColumnIndex(OpenableColumns.SIZE)
                if (index >= 0 && !it.isNull(index)) {
                    return it.getLong(index).toULong()
                }
            }
        }
        return null
    }

    private fun resolveContentPath(context: Context, uri: Uri): String? {
        when (uri.authority) {
            EXTERNAL_STORAGE_AUTHORITY -> {
                val docId = DocumentsContract.getDocumentId(uri)
                if (docId.startsWith(PRIMARY_VOLUME_DOC_ID_PREFIX)) {
                    return "$LOCAL_PREFIX/${docId.removePrefix(PRIMARY_VOLUME_DOC_ID_PREFIX)}"
                }
                // Removable volume (e.g. "XXXX-XXXX:...") — unsupported.
                return null
            }

            MEDIA_AUTHORITY -> {
                // MediaStore URIs: the `_data` column carries the absolute path.
                val cursor = queryColumns(context, uri, arrayOf(DATA_COLUMN)) ?: return null
                cursor.use {
                    if (it.moveToFirst()) {
                        val index = it.getColumnIndex(DATA_COLUMN)
                        if (index >= 0 && !it.isNull(index)) {
                            return it.getString(index)
                        }
                    }
                }
                return null
            }

            else -> return null
        }
    }

    private fun queryColumns(context: Context, uri: Uri, columns: Array<String>): Cursor? {
        return try {
            context.contentResolver.query(uri, columns, null, null, null)
        } catch (e: Exception) {
            null
        }
    }
}
