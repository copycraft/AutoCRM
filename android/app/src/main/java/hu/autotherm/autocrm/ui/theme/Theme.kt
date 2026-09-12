package hu.autotherm.autocrm.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The same palette as the web client (frontend/tailwind.config.ts). Not "close to": the
 * fitter and the office look at the same order from two devices, and a plate badge that is
 * orange here and red there is a bug report waiting to happen.
 */
val Panel = Color(0xFFF7F8F7)
val Surface = Color(0xFFFFFFFF)
val Steel900 = Color(0xFF1B2327)
val Steel500 = Color(0xFF6B767C)
val Steel200 = Color(0xFFD5DBDC)
val Signal = Color(0xFFE8590C)
val Cold = Color(0xFF0F5C7A)
val Done = Color(0xFF2F7A3E)

/**
 * Light only, deliberately. The web client is light only, the app is used under workshop
 * lighting where a dark surface with a glossy screen is harder to read, and a second theme
 * is a second set of contrast decisions nobody will check.
 */
private val AutoCrmColors = lightColorScheme(
    primary = Steel900,
    onPrimary = Surface,
    primaryContainer = Steel200,
    onPrimaryContainer = Steel900,
    secondary = Cold,
    onSecondary = Surface,
    background = Panel,
    onBackground = Steel900,
    surface = Surface,
    onSurface = Steel900,
    surfaceVariant = Panel,
    onSurfaceVariant = Steel500,
    outline = Steel200,
    outlineVariant = Steel200,
    error = Signal,
    onError = Surface,
    errorContainer = Color(0xFFFDEEE4),
    onErrorContainer = Signal,
)

/**
 * The web client's type scale (metadata 12.8 / body 16 / section 20 / record 25 / page 31),
 * mapped onto the Material 3 roles the components actually read. IBM Plex is not bundled —
 * it would add ~400 KB per weight for a difference nobody on a phone will notice — so the
 * platform sans is used and only the sizes and spacing carry over.
 */
private val Plex = FontFamily.Default
private val Mono = FontFamily.Monospace

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
fun AutoCrmTheme(content: @Composable () -> Unit) {
    // isSystemInDarkTheme is read and ignored on purpose: the call documents that the
    // decision was made rather than forgotten.
    @Suppress("UNUSED_EXPRESSION")
    isSystemInDarkTheme()
    MaterialTheme(
        colorScheme = AutoCrmColors,
        typography = AutoCrmTypography,
        content = content,
    )
}
