package hu.autotherm.autocrm.data.prefs

import android.content.Context
import android.content.res.Configuration
import kotlin.math.roundToInt

/**
 * How big the app draws, on top of the phone's own display and font size: a fitter in
 * gloves wants bigger targets, the office on a small phone wants more rows on screen.
 *
 * Applied as an override of the activity's configuration (density and font scale), not as
 * a Compose `LocalDensity`: dialogs, menus and date pickers are separate windows that
 * re-read the density from their context, and a Compose-only scale would leave every one
 * of them at the old size. The price is that a change takes effect by recreating the
 * activity, which is why the settings screen previews before it applies.
 *
 * SharedPreferences rather than the DataStore the theme uses: `attachBaseContext` runs
 * before anything asynchronous could answer, so the value has to be readable in place.
 * Device-wide and kept across sign-outs, like the theme.
 */
object DisplayScale {
    private const val PREFS = "display"
    private const val KEY_UI = "ui_scale"
    private const val KEY_TEXT = "text_scale"

    /** Everything: spacing, icons, buttons and text together (like pinching the whole app). */
    val UI_STEPS = listOf(0.8f, 0.85f, 0.9f, 0.95f, 1f, 1.05f, 1.1f, 1.15f, 1.2f, 1.25f, 1.3f)

    /** Text only, multiplied onto the phone's own font size setting. */
    val TEXT_STEPS = listOf(0.85f, 0.9f, 0.95f, 1f, 1.05f, 1.1f, 1.15f, 1.2f, 1.3f, 1.4f)

    data class Scale(val ui: Float = 1f, val text: Float = 1f) {
        val isDefault: Boolean get() = ui == 1f && text == 1f
    }

    fun read(context: Context): Scale {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        return Scale(
            ui = snap(prefs.getFloat(KEY_UI, 1f), UI_STEPS),
            text = snap(prefs.getFloat(KEY_TEXT, 1f), TEXT_STEPS),
        )
    }

    /** Synchronous on purpose: the caller recreates the activity right after. */
    fun save(context: Context, scale: Scale) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putFloat(KEY_UI, snap(scale.ui, UI_STEPS))
            .putFloat(KEY_TEXT, snap(scale.text, TEXT_STEPS))
            .commit()
    }

    /**
     * The configuration delta for [base], or null when nothing is scaled. Only the fields
     * set here override the system's; the screen size in dp shrinks with the density so
     * layouts that read it (dialog heights) agree with what is drawn.
     */
    fun overrideFor(base: Context): Configuration? {
        val scale = read(base)
        if (scale.isDefault) return null
        val system = base.resources.configuration
        return Configuration().apply {
            if (scale.ui != 1f && system.densityDpi != Configuration.DENSITY_DPI_UNDEFINED) {
                densityDpi = (system.densityDpi * scale.ui).roundToInt()
                screenWidthDp = (system.screenWidthDp / scale.ui).roundToInt()
                screenHeightDp = (system.screenHeightDp / scale.ui).roundToInt()
                smallestScreenWidthDp = (system.smallestScreenWidthDp / scale.ui).roundToInt()
            }
            if (scale.text != 1f) fontScale = system.fontScale * scale.text
        }
    }

    /** A stored value that is not one of the steps (an older build's) reads as the nearest. */
    private fun snap(value: Float, steps: List<Float>): Float =
        steps.minBy { kotlin.math.abs(it - value) }
}
