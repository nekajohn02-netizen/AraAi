package ai.ara.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Stores API keys encrypted with an AES-256-GCM key that lives in Android
 * Keystore (non-exportable). Ciphertext + IV are kept in private
 * SharedPreferences, which are excluded from backup. Keys are never logged
 * and never written to the Rust data directory.
 */
class KeyVault(context: Context) {
    private val prefs = context.getSharedPreferences("ara_vault", Context.MODE_PRIVATE)

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return gen.generateKey()
    }

    fun put(providerId: String, secret: String) {
        val c = Cipher.getInstance(TRANSFORM)
        c.init(Cipher.ENCRYPT_MODE, key())
        val ct = c.doFinal(secret.toByteArray(Charsets.UTF_8))
        prefs.edit()
            .putString("$providerId.iv", Base64.encodeToString(c.iv, Base64.NO_WRAP))
            .putString("$providerId.ct", Base64.encodeToString(ct, Base64.NO_WRAP))
            .apply()
    }

    fun get(providerId: String): String? {
        val iv = prefs.getString("$providerId.iv", null) ?: return null
        val ct = prefs.getString("$providerId.ct", null) ?: return null
        return try {
            val c = Cipher.getInstance(TRANSFORM)
            c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP)))
            String(c.doFinal(Base64.decode(ct, Base64.NO_WRAP)), Charsets.UTF_8)
        } catch (e: Exception) {
            null // key invalidated (e.g. device reset): ask the user again
        }
    }

    fun has(providerId: String) = prefs.contains("$providerId.ct")

    fun remove(providerId: String) {
        prefs.edit().remove("$providerId.iv").remove("$providerId.ct").apply()
    }

    private companion object {
        const val ALIAS = "ara_api_keys"
        const val TRANSFORM = "AES/GCM/NoPadding"
    }
}
