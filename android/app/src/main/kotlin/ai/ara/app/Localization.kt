package ai.ara.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import java.util.Locale

/**
 * Reads [LocalConfiguration] first so these recompose correctly if the
 * system language changes while Ara is running. A bare `Locale.getDefault()`
 * read is not observable to Compose (Android Lint's `NonObservableLocale`)
 * and would silently keep showing the old language's content until the
 * next unrelated recomposition happened to catch up.
 */
@Composable
fun isArabicLocale(): Boolean {
    val configuration = LocalConfiguration.current
    return remember(configuration) { Locale.getDefault().language == "ar" }
}

@Composable
fun currentLanguageTag(): String {
    val configuration = LocalConfiguration.current
    return remember(configuration) { Locale.getDefault().language }
}
