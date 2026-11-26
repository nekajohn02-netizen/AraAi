package ai.ara.app

import android.content.Context
import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/**
 * True when motion should be minimized app-wide. Two independent sources,
 * either one is enough:
 *  - Ara's own "Reduce motion" switch in Settings (per-app, always available).
 *  - The system's "Remove animations" developer setting
 *    (`ANIMATOR_DURATION_SCALE == 0`) — the de-facto signal Android apps
 *    check in the absence of a direct platform "Reduce Motion" API.
 * Every AnimatedVisibility/AnimatedContent/animateContentSize call in the app
 * should read [motionMillis] rather than hardcoding a duration, so turning
 * this on actually removes the animation instead of just hiding its visible
 * effect behind a near-zero number.
 */
val LocalReduceMotion = compositionLocalOf { false }

private const val PREFS = "ara_ui_prefs"
private const val KEY_REDUCE_MOTION = "reduce_motion"

fun readReduceMotionPref(ctx: Context): Boolean =
    ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(KEY_REDUCE_MOTION, false)

fun writeReduceMotionPref(ctx: Context, value: Boolean) {
    ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean(KEY_REDUCE_MOTION, value).apply()
}

@Composable
fun systemRemovesAnimations(): Boolean {
    val ctx = LocalContext.current
    return remember {
        runCatching {
            Settings.Global.getFloat(ctx.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
        }.getOrDefault(false)
    }
}

/** Short, consistent animation duration — 0 (i.e. no animation) when
 *  [LocalReduceMotion] is on, so the UI stays fully understandable with
 *  motion disabled rather than just playing it faster. */
@Composable
fun motionMillis(normal: Int = 200): Int = if (LocalReduceMotion.current) 0 else normal
