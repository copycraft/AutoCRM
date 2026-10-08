package hu.autotherm.autocrm.util

/**
 * A VIN as the speech recognizer heard it ("vé dé bé kilenc nulla hat…", "W D B 906…",
 * "dupla vé") turned into the 17 characters it stands for. Letter and digit names in
 * Hungarian and English are understood; I, O and Q (never in a VIN) become 1, 0 and 0.
 */
fun spokenToVin(spoken: String): String {
    val words = spoken.lowercase()
        .replace("dupla vé", "duplavé")
        .replace("dupla v", "duplavé")
        .replace("double u", "w")
        .split(Regex("[\\s,.;:-]+"))
        .filter { it.isNotBlank() }
    val out = StringBuilder()
    for (w in words) {
        val mapped = NAMES[w]
        when {
            mapped != null -> out.append(mapped)
            // "WDB906" said in one breath arrives as one token.
            w.all { it.isLetterOrDigit() } -> out.append(w.uppercase())
        }
    }
    return out.toString()
        .map { c ->
            when (c) {
                'I' -> '1'
                'O', 'Q' -> '0'
                else -> c
            }
        }
        .filter { it in 'A'..'Z' || it in '0'..'9' }
        .joinToString("")
        .take(17)
}

private val NAMES: Map<String, String> = mapOf(
    // Hungarian letter names.
    "á" to "A", "a" to "A", "bé" to "B", "be" to "B", "cé" to "C", "ce" to "C", "dé" to "D", "de" to "D",
    "é" to "E", "e" to "E", "ef" to "F", "gé" to "G", "ge" to "G", "há" to "H", "ha" to "H",
    "i" to "I", "jé" to "J", "je" to "J", "ká" to "K", "ka" to "K", "el" to "L", "em" to "M",
    "en" to "N", "ó" to "O", "o" to "O", "pé" to "P", "pe" to "P", "kú" to "Q", "ku" to "Q", "er" to "R",
    "es" to "S", "té" to "T", "te" to "T", "ú" to "U", "u" to "U", "vé" to "V", "ve" to "V",
    "duplavé" to "W", "iksz" to "X", "ipszilon" to "Y", "zé" to "Z", "ze" to "Z",
    // Hungarian digits.
    "nulla" to "0", "zéró" to "0", "egy" to "1", "kettő" to "2", "két" to "2", "három" to "3",
    "négy" to "4", "öt" to "5", "hat" to "6", "hét" to "7", "nyolc" to "8", "kilenc" to "9",
    // English digits and the letter names a recognizer spells out.
    "zero" to "0", "one" to "1", "two" to "2", "three" to "3", "four" to "4", "five" to "5",
    "six" to "6", "seven" to "7", "eight" to "8", "nine" to "9",
    "bee" to "B", "see" to "C", "dee" to "D", "ef" to "F", "gee" to "G", "aitch" to "H",
    "jay" to "J", "kay" to "K", "ell" to "L", "pee" to "P", "cue" to "Q", "ar" to "R",
    "ess" to "S", "tee" to "T", "you" to "U", "vee" to "V", "ex" to "X", "why" to "Y", "zed" to "Z", "zee" to "Z",
)

/** Seventeen characters a VIN may use: the moment to look it up. */
fun isCompleteVin(vin: String): Boolean =
    vin.length == 17 && vin.all { (it in 'A'..'Z' && it !in "IOQ") || it in '0'..'9' }
