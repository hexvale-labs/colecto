package app.colecto.mobile

import android.app.Activity
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.view.HapticFeedbackConstants
import android.view.SoundEffectConstants
import android.webkit.WebView
import androidx.activity.BackEventCompat
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.util.concurrent.Executors
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class PressArgs {
    var kind: String? = null
}

@InvokeArg
class SecretIdArgs {
    var id: String? = null
}

@InvokeArg
class SecretSetArgs {
    var id: String? = null
    var value: String? = null
}

/**
 * Back gestures, press feedback and Android Keystore credential storage for Colecto.
 *
 * Colecto keeps its own screens (an open message, Settings, a dialog) in the WebView's history,
 * so "can the page go back" is exactly "is there something in the app to close".
 *
 * The `secret*` commands are not listed in `build.rs` or any WebView capability, so Tauri's ACL
 * refuses calls from page JavaScript; only Rust code holding the plugin handle can invoke them.
 */
@TauriPlugin
class NativePlugin(private val activity: Activity) : Plugin(activity) {
    private val appContext = activity.applicationContext
    private var webView: WebView? = null

    // Serializes Keystore and disk access off the UI thread so startup and saves never block
    // rendering or race on first-time key generation.
    private val secretExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "colecto-keystore").apply { isDaemon = true }
    }

    // While the page can go back, this callback owns the gesture and tells the page how far it
    // has travelled, so the page can shrink the screen under the finger like a native app.
    // Otherwise it hands the press on to Tauri's own callback, which leaves the app.
    private val back = object : OnBackPressedCallback(true) {
        private var tracking = false

        override fun handleOnBackStarted(backEvent: BackEventCompat) {
            tracking = webView?.canGoBack() == true
            if (tracking) send("start", backEvent)
        }

        override fun handleOnBackProgressed(backEvent: BackEventCompat) {
            if (tracking) send("progress", backEvent)
        }

        override fun handleOnBackCancelled() {
            if (tracking) send("cancel", null)
            tracking = false
        }

        override fun handleOnBackPressed() {
            tracking = false
            val view = webView
            if (view != null && view.canGoBack()) {
                view.goBack()
                return
            }
            isEnabled = false
            activity.onBackPressed()
            isEnabled = true
        }
    }

    override fun load(webView: WebView) {
        this.webView = webView
        // Registered after Tauri's own callback (added when its plugin was created), so Android
        // asks this one first.
        activity.runOnUiThread {
            (activity as ComponentActivity).onBackPressedDispatcher.addCallback(activity, back)
        }
    }

    private fun send(phase: String, event: BackEventCompat?) {
        val progress = event?.progress ?: 0f
        val edge = if (event?.swipeEdge == BackEventCompat.EDGE_RIGHT) "right" else "left"
        val detail = "{\"phase\":\"$phase\",\"progress\":$progress,\"edge\":\"$edge\"}"
        webView?.evaluateJavascript(
            "window.dispatchEvent(new CustomEvent('colecto:back',{detail:$detail}))",
            null,
        )
    }

    /** A tap on a control: the system click, plus a tick for switches and choices. */
    @Command
    fun press(invoke: Invoke) {
        val args = invoke.parseArgs(PressArgs::class.java)
        activity.runOnUiThread {
            webView?.let { view ->
                view.playSoundEffect(SoundEffectConstants.CLICK)
                if (args.kind == "toggle") view.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
            }
        }
        invoke.resolve()
    }

    /** Read one vault wrapping entry encrypted with Android Keystore. */
    @Command
    fun secretGet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretIdArgs::class.java)
        secretExecutor.execute {
            try {
                val id = requireValidId(args.id)
                val file = entryFile(id)
                if (!file.isFile) {
                    invoke.reject("Credential entry not found", "missing")
                    return@execute
                }
                val blob = file.readBytes()
                if (blob.size < 2) {
                    throw GeneralSecurityException("Damaged Android Keystore entry")
                }
                val ivLen = blob[1].toInt() and 0xFF
                if (blob[0] != FORMAT_VERSION || ivLen < 12 || ivLen > 16 || blob.size <= 2 + ivLen + 16) {
                    throw GeneralSecurityException("Damaged Android Keystore entry")
                }
                val iv = blob.copyOfRange(2, 2 + ivLen)
                val ciphertext = blob.copyOfRange(2 + ivLen, blob.size)
                val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
                cipher.init(Cipher.DECRYPT_MODE, wrappingKey(create = false), GCMParameterSpec(GCM_TAG_BITS, iv))
                cipher.updateAAD(entryAad(id))
                val plaintext = cipher.doFinal(ciphertext)
                val value = try {
                    String(plaintext, Charsets.UTF_8)
                } finally {
                    plaintext.fill(0)
                }
                invoke.resolve(JSObject().put("value", value))
            } catch (error: Exception) {
                invoke.reject(error.message ?: error.javaClass.simpleName)
            }
        }
    }

    /** Encrypt and atomically persist one vault wrapping entry under Android Keystore. */
    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretSetArgs::class.java)
        secretExecutor.execute {
            try {
                val id = requireValidId(args.id)
                val value = args.value
                if (value.isNullOrEmpty()) {
                    throw IllegalArgumentException("Empty credential value")
                }
                val plaintext = value.toByteArray(Charsets.UTF_8)
                val iv: ByteArray
                val ciphertext: ByteArray
                try {
                    val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
                    cipher.init(Cipher.ENCRYPT_MODE, wrappingKey(create = true))
                    cipher.updateAAD(entryAad(id))
                    iv = cipher.iv
                    if (iv == null || iv.size !in 12..16) {
                        throw GeneralSecurityException("Unexpected AES-GCM IV length")
                    }
                    ciphertext = cipher.doFinal(plaintext)
                } finally {
                    plaintext.fill(0)
                }
                val blob = ByteArray(2 + iv.size + ciphertext.size)
                blob[0] = FORMAT_VERSION
                blob[1] = iv.size.toByte()
                iv.copyInto(blob, destinationOffset = 2)
                ciphertext.copyInto(blob, destinationOffset = 2 + iv.size)

                val dir = secretsDir()
                if (!dir.exists() && !dir.mkdirs()) {
                    throw IOException("Could not create credential storage directory")
                }
                val tmp = File(dir, "$id.tmp")
                val target = File(dir, id)
                try {
                    FileOutputStream(tmp).use { out ->
                        out.write(blob)
                        out.flush()
                        out.fd.sync()
                    }
                    if (!tmp.renameTo(target)) {
                        throw IOException("Could not save credential entry")
                    }
                } catch (error: Exception) {
                    tmp.delete()
                    throw error
                }
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: error.javaClass.simpleName)
            }
        }
    }

    /** Delete one vault wrapping entry; removing an absent entry succeeds. */
    @Command
    fun secretDelete(invoke: Invoke) {
        val args = invoke.parseArgs(SecretIdArgs::class.java)
        secretExecutor.execute {
            try {
                val id = requireValidId(args.id)
                val dir = secretsDir()
                File(dir, "$id.tmp").delete()
                val target = File(dir, id)
                if (target.exists() && !target.delete() && target.exists()) {
                    throw IOException("Could not remove credential entry")
                }
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: error.javaClass.simpleName)
            }
        }
    }

    private fun secretsDir(): File = File(appContext.noBackupFilesDir, SECRETS_DIR)

    private fun entryFile(id: String): File = File(secretsDir(), id)

    private fun requireValidId(id: String?): String {
        if (id == null || !VALID_ID.matches(id)) {
            throw IllegalArgumentException("Invalid credential entry identifier")
        }
        return id
    }

    private fun entryAad(id: String): ByteArray = "colecto-secret-v1:$id".toByteArray(Charsets.UTF_8)

    private fun wrappingKey(create: Boolean): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        val existing = keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        if (existing != null) return existing
        if (!create) {
            throw GeneralSecurityException("Android Keystore key missing")
        }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val spec = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setRandomizedEncryptionRequired(true)
            .build()
        generator.init(spec)
        return generator.generateKey()
    }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val KEY_ALIAS = "app.colecto.vault.key.v1"
        private const val CIPHER_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val SECRETS_DIR = "colecto-secrets"
        private const val FORMAT_VERSION: Byte = 1
        private const val GCM_TAG_BITS = 128
        private val VALID_ID = Regex("^[0-9a-f]{32}$")
    }
}
