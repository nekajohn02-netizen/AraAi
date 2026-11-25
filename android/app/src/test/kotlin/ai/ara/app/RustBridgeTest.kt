package ai.ara.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.ara_ffi.AraCore
import uniffi.ara_ffi.FfiException
import uniffi.ara_ffi.collectDeviceProfile
import uniffi.ara_ffi.coreVersion
import uniffi.ara_ffi.selectModels
import uniffi.ara_ffi.validateCatalog

/**
 * Exercises the real Kotlin -> UniFFI -> Rust path on the JVM, loading the
 * host build of libara_ffi.so through JNA (see `jna.library.path` in Gradle).
 */
class RustBridgeTest {

    @Test
    fun coreVersionCrossesTheBridge() {
        assertEquals("0.1.0", coreVersion())
    }

    @Test
    fun deviceProfileIsReadFromTheHost() {
        val dir = System.getProperty("java.io.tmpdir")
        val json = collectDeviceProfile(dir, "{}")
        assertTrue(json, json.contains("\"available_ram_bytes\""))
        assertTrue(json, json.contains("\"cpu_cores\""))
    }

    @Test
    fun emptyCatalogSelectsNothing() {
        val catalog = """{"schema_version":1,"models":[]}"""
        assertEquals(0u, validateCatalog(catalog))
        val device = collectDeviceProfile(System.getProperty("java.io.tmpdir"), "{}")
        val req = """{"mode":"speed","capability":"chat","language":"ar","context":1024}"""
        val out = selectModels(catalog, device, req, "[]")
        assertTrue(out, out.contains("\"candidates\":[]"))
    }

    @Test
    fun rustErrorsBecomeTypedKotlinExceptions() {
        try {
            validateCatalog("{not json")
            fail("expected FfiException")
        } catch (e: FfiException.InvalidInput) {
            assertTrue(e.detail, e.detail.contains("catalog json"))
        }
    }

    @Test
    fun araCoreWorksAcrossTheBridge() {
        val dir = java.nio.file.Files.createTempDirectory("ara").toString()
        val core = AraCore(dir)
        val models = core.models("{}", "automatic", "ar", "software_engineer")
        assertTrue(models, models.contains("qwen2.5-coder-1.5b-instruct-q4_k_m"))
        val id = core.createConversation("اختبار", "security")
        assertTrue(core.conversations().contains(id))
        core.renameConversation(id, "جديد")
        assertTrue(core.conversation(id).contains("جديد"))
        core.setPermission("alarms", "always")
        assertTrue(core.permissions().contains("alarms"))
        try {
            core.sendMessage(id, "hi", object : uniffi.ara_ffi.TextListener {
                override fun onText(text: String) {}
            }, uniffi.ara_ffi.CancelHandle())
            fail("expected: no model loaded")
        } catch (e: FfiException.InvalidInput) {
            assertTrue(e.detail.contains("no model"))
        }
        core.deleteConversation(id)
        assertTrue(!core.conversations().contains(id))
    }

    @Test
    fun documentsSearchAndMemoryAcrossTheBridge() {
        val core = AraCore(java.nio.file.Files.createTempDirectory("ara").toString())
        core.importDocument("ملاحظات.txt", "موعد الدكتور يوم الأحد\nالمفتاح مع أحمد".toByteArray())
        val hits = core.searchDocuments("احمد")
        assertTrue(hits, hits.contains("ملاحظات.txt") && hits.contains("\"line\":2"))
        core.addMemory("بحب القهوة")
        assertTrue(core.memories().contains("القهوة"))
        try {
            core.importDocument("x.pdf", byteArrayOf(1, 2))
            fail("pdf not supported yet")
        } catch (e: FfiException) {
        }
    }

    @Test
    fun apiHubEnforcesConsent() {
        val core = AraCore(java.nio.file.Files.createTempDirectory("ara").toString())
        core.saveProvider("""{"id":"kimi","name":"Kimi","base_url":"https://api.moonshot.ai/v1","model":"kimi-k3"}""")
        try {
            core.useCloudModel("kimi", "sk-test-0000000000000")
            fail("consent required")
        } catch (e: FfiException.InvalidInput) {
            assertTrue(e.detail.contains("consent"))
        }
        core.setCloudConsent("kimi", true)
        core.useCloudModel("kimi", "sk-test-0000000000000")
        assertEquals("cloud:kimi/kimi-k3", core.loadedModel())
    }

    @Test
    fun missionsAndSecurityAcrossTheBridge() {
        val core = AraCore(java.nio.file.Files.createTempDirectory("ara").toString())
        val m = core.createMission("راجع الملفات", "analyst", 10u, 60u)
        assertTrue(m, m.contains("\"status\":\"running\""))
        try {
            core.createMission("x", "general", 5u, 2000u)
            fail("24h cap")
        } catch (e: FfiException) {
        }
        val r = core.securityCheck("""{"screen_lock_set":false}""")
        assertTrue(r, r.contains("no_screen_lock"))
    }
}
