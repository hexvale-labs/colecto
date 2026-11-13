package app.colecto.mobile

import android.app.Activity
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
import app.tauri.plugin.Plugin

@InvokeArg
class PressArgs {
    var kind: String? = null
}

/**
 * Back gestures and press feedback for Colecto's page.
 *
 * Colecto keeps its own screens (an open message, Settings, a dialog) in the WebView's history,
 * so "can the page go back" is exactly "is there something in the app to close".
 */
@TauriPlugin
class NativePlugin(private val activity: Activity) : Plugin(activity) {
    private var webView: WebView? = null

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
}
