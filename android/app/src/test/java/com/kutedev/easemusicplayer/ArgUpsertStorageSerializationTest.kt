package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.singleton.types.ArgUpsertStorage
import com.kutedev.easemusicplayer.singleton.types.StorageId
import com.kutedev.easemusicplayer.singleton.types.StorageType
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Verifies that ArgUpsertStorage serializes ALL fields (including
 * defaults) so Rust's strict deserializer sees every required field.
 *
 * Regression test for the bug where `encodeDefaults = false` caused
 * `missing field isAnonymous` / `missing field username` errors.
 */
class ArgUpsertStorageSerializationTest {

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    @Test
    fun `all fields serialized when values match defaults`() {
        val arg = ArgUpsertStorage(
            id = null,
            addr = "",
            alias = "",
            username = "",
            password = "",
            isAnonymous = false,
            typ = StorageType.WEBDAV,
        )
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject

        // Every Rust field must be present.
        val keys = element.keys
        assertTrue("missing addr", "addr" in keys)
        assertTrue("missing alias", "alias" in keys)
        assertTrue("missing username", "username" in keys)
        assertTrue("missing password", "password" in keys)
        assertTrue("missing isAnonymous", "isAnonymous" in keys)
        assertTrue("missing typ", "typ" in keys)
        // id is Option<StorageId>; with encodeDefaults=true it should
        // also be present (as null).
        assertTrue("missing id", "id" in keys)
    }

    @Test
    fun `isAnonymous false is sent as false`() {
        val arg = ArgUpsertStorage(isAnonymous = false)
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject
        assertEquals(false, element["isAnonymous"]!!.jsonPrimitive.boolean)
    }

    @Test
    fun `typ webdav is sent as WEBDAV`() {
        val arg = ArgUpsertStorage(typ = StorageType.WEBDAV)
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject
        assertEquals("WEBDAV", element["typ"]!!.jsonPrimitive.content)
    }

    @Test
    fun `non-default values are preserved`() {
        val arg = ArgUpsertStorage(
            addr = "https://dav.example.com",
            alias = "mywebdav",
            isAnonymous = true,
        )
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject
        assertEquals("https://dav.example.com", element["addr"]!!.jsonPrimitive.content)
        assertEquals("mywebdav", element["alias"]!!.jsonPrimitive.content)
        assertEquals(true, element["isAnonymous"]!!.jsonPrimitive.boolean)
    }

    @Test
    fun `id null is sent as JSON null`() {
        val arg = ArgUpsertStorage(id = null)
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject
        // id should be present (key exists) and value should be JsonNull
        assertNotNull(element["id"])
        assertTrue(element["id"]!!.toString() == "null")
    }

    @Test
    fun `id value is sent as bare number`() {
        val arg = ArgUpsertStorage(id = StorageId(42))
        val element = json.encodeToJsonElement(
            ArgUpsertStorage.serializer(),
            arg,
        ) as JsonObject
        // StorageId is transparent — serializes to bare number.
        assertEquals("42", element["id"]!!.jsonPrimitive.content)
    }
}
