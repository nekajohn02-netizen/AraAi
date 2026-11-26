package ai.ara.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics

/**
 * Marks an error/notice banner as a TalkBack live region: when its text
 * appears or changes, a screen reader announces it without the user having
 * to navigate to it first. Every error/notice `Text` in the app should carry
 * this — previously none did, so a screen-reader user had no way to learn a
 * request failed unless they happened to explore to that exact spot.
 */
fun Modifier.liveRegionAnnounce(): Modifier = semantics { liveRegion = LiveRegionMode.Polite }
