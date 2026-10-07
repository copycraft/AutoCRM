package hu.autotherm.autocrm.util

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast

/**
 * Hands an address or a point to the phone's map app (0049): turn-by-turn to a customer's
 * site, or where an inspection photo was taken. Google Maps' navigation scheme first, the
 * generic `geo:` one for any other map app.
 */
fun openNavigation(context: Context, address: String) {
    val q = Uri.encode(address)
    val attempts = listOf(
        Intent(Intent.ACTION_VIEW, Uri.parse("google.navigation:q=$q")),
        Intent(Intent.ACTION_VIEW, Uri.parse("geo:0,0?q=$q")),
        Intent(Intent.ACTION_VIEW, Uri.parse("https://www.google.com/maps/dir/?api=1&destination=$q")),
    )
    launchFirst(context, attempts)
}

fun openMapAt(context: Context, lat: Double, lon: Double, label: String) {
    val attempts = listOf(
        Intent(Intent.ACTION_VIEW, Uri.parse("geo:$lat,$lon?q=$lat,$lon(${Uri.encode(label)})")),
        Intent(Intent.ACTION_VIEW, Uri.parse("https://www.google.com/maps/search/?api=1&query=$lat,$lon")),
    )
    launchFirst(context, attempts)
}

private fun launchFirst(context: Context, attempts: List<Intent>) {
    for (intent in attempts) {
        try {
            context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return
        } catch (_: ActivityNotFoundException) {
            // try the next form
        }
    }
    Toast.makeText(context, "Nincs térképalkalmazás a telefonon.", Toast.LENGTH_SHORT).show()
}
