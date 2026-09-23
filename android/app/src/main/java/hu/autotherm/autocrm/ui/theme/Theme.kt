package hu.autotherm.autocrm.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import hu.autotherm.autocrm.R

/**
 * The same palette as the web client (frontend/tailwind.config.ts). Not "close to": the
 * fitter and the office look at the same order from two devices, and a plate badge that is
 * orange here and red there is a bug report waiting to happen.
 *
 * The names below stay fixed so every screen keeps reading `Steel900` for text and
 * `Surface` for cards; what changes with the theme is what those names resolve to.
 * That is the whole dark-theme migration: no call site knows which mode is on.
 */
private object ThemeFlags {
    var dark: Boolean = false
    var amoled: Boolean = false
}

private val LightPanel = Color(0xFFF7F8F7)
private val LightSurface = Color(0xFFFFFFFF)
private val LightInk = Color(0xFF1B2327)
private val LightSecondary = Color(0xFF6B767C)
private val LightLine = Color(0xFFD5DBDC)

/** Ink on dark: near-white. Surfaces reuse the light ink so cards lift off the page. */
private val DarkBg = Color(0xFF101415)
private val DarkSurface = Color(0xFF1B2327)
private val DarkInk = Color(0xFFECEFF0)
private val DarkSecondary = Color(0xFF9AA6AC)
private val DarkLine = Color(0xFF303B41)

private val AmoledBg = Color(0xFF000000)
private val AmoledSurface = Color(0xFF101314)
private val AmoledLine = Color(0xFF262626)

val Panel: Color
    get() = when {
        !ThemeFlags.dark -> LightPanel
        ThemeFlags.amoled -> AmoledBg
        else -> DarkBg
    }
val Surface: Color
    get() = when {
        !ThemeFlags.dark -> LightSurface
        ThemeFlags.amoled -> AmoledSurface
        else -> DarkSurface
    }
val Steel900: Color
    get() = if (ThemeFlags.dark) DarkInk else LightInk
val Steel500: Color
    get() = if (ThemeFlags.dark) DarkSecondary else LightSecondary
val Steel200: Color
    get() = when {
        !ThemeFlags.dark -> LightLine
        ThemeFlags.amoled -> AmoledLine
        else -> DarkLine
    }

/** Brand accents hold on both themes; the cool tone is lifted for dark contrast. */
val Signal = Color(0xFFE8590C)
val Cold: Color
    get() = if (ThemeFlags.dark) Color(0xFF5BA3C4) else Color(0xFF0F5C7A)
val Done: Color
    get() = if (ThemeFlags.dark) Color(0xFF6FA876) else Color(0xFF2F7A3E)

private val AutoCrmLightColors = lightColorScheme(
    primary = LightInk,
    onPrimary = LightSurface,
    primaryContainer = LightLine,
    onPrimaryContainer = LightInk,
    secondary = Color(0xFF0F5C7A),
    onSecondary = LightSurface,
    background = LightPanel,
    onBackground = LightInk,
    surface = LightSurface,
    onSurface = LightInk,
    surfaceVariant = LightPanel,
    onSurfaceVariant = LightSecondary,
    outline = LightLine,
    outlineVariant = LightLine,
    error = Signal,
    onError = LightSurface,
    errorContainer = Color(0xFFFDEEE4),
    onErrorContainer = Signal,
)

private fun autoCrmDarkColors(amoled: Boolean) = darkColorScheme(
    primary = DarkInk,
    onPrimary = if (amoled) AmoledSurface else DarkSurface,
    primaryContainer = if (amoled) AmoledLine else DarkLine,
    onPrimaryContainer = DarkInk,
    secondary = Color(0xFF5BA3C4),
    onSecondary = Color(0xFF101415),
    background = if (amoled) AmoledBg else DarkBg,
    onBackground = DarkInk,
    surface = if (amoled) AmoledSurface else DarkSurface,
    onSurface = DarkInk,
    surfaceVariant = if (amoled) AmoledBg else DarkBg,
    onSurfaceVariant = DarkSecondary,
    outline = if (amoled) AmoledLine else DarkLine,
    outlineVariant = if (amoled) AmoledLine else DarkLine,
    error = Signal,
    onError = Color(0xFF101415),
    errorContainer = Color(0xFF3A2415),
    onErrorContainer = Color(0xFFFFB59D),
)

/**
 * IBM Plex Sans + Mono, the web client's fonts, bundled as res/font TTFs (OFL, same source as
 * next/font). Sans is the variable [wdth,wght] build; Compose resolves the Medium/SemiBold
 * named instances by weight. Downloadable fonts were rejected: the app must render correctly
 * in a basement garage with no signal.
 */
private val Plex = FontFamily(
    Font(R.font.plex_sans_variable, FontWeight.Normal),
    Font(R.font.plex_sans_variable, FontWeight.Medium),
    Font(R.font.plex_sans_variable, FontWeight.SemiBold),
    // The variable file spans 100–700: without an explicit Bold entry M3 components
    // asking for Bold fall back to another family and the typeface visibly mixes.
    Font(R.font.plex_sans_variable, FontWeight.Bold),
)
private val Mono = FontFamily(
    Font(R.font.plex_mono_regular, FontWeight.Normal),
)

val AutoCrmTypography = Typography(
    displaySmall = TextStyle(fontFamily = Plex, fontSize = 31.sp, lineHeight = 37.sp, fontWeight = FontWeight.SemiBold),
    headlineMedium = TextStyle(fontFamily = Plex, fontSize = 25.sp, lineHeight = 33.sp, fontWeight = FontWeight.SemiBold),
    titleLarge = TextStyle(fontFamily = Plex, fontSize = 20.sp, lineHeight = 28.sp, fontWeight = FontWeight.SemiBold),
    titleMedium = TextStyle(fontFamily = Plex, fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.Medium),
    bodyLarge = TextStyle(fontFamily = Plex, fontSize = 16.sp, lineHeight = 24.sp),
    bodyMedium = TextStyle(fontFamily = Plex, fontSize = 16.sp, lineHeight = 24.sp),
    labelLarge = TextStyle(fontFamily = Plex, fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.Medium),
    labelMedium = TextStyle(fontFamily = Plex, fontSize = 13.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    labelSmall = TextStyle(fontFamily = Plex, fontSize = 13.sp, lineHeight = 20.sp),
)

/** Numbers that are compared by eye — plates, VINs, order numbers, money — are monospaced. */
val MonoStyle = TextStyle(fontFamily = Mono, fontSize = 16.sp, lineHeight = 24.sp)
val MonoSmall = TextStyle(fontFamily = Mono, fontSize = 13.sp, lineHeight = 20.sp)

@Composable
fun AutoCrmTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    amoled: Boolean = false,
    content: @Composable () -> Unit,
) {
    // Flags first: every palette read below (and on every screen) resolves through
    // them, so a theme flip recomposes the whole tree consistently.
    ThemeFlags.dark = darkTheme
    ThemeFlags.amoled = amoled && darkTheme
    MaterialTheme(
        colorScheme = if (darkTheme) autoCrmDarkColors(amoled && darkTheme) else AutoCrmLightColors,
        typography = AutoCrmTypography,
        content = content,
    )
}
