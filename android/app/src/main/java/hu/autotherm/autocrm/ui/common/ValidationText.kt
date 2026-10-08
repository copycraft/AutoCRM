package hu.autotherm.autocrm.ui.common

/**
 * The server's validation messages are English ("partner_id is required", "email is not a
 * valid address"). Shown as they are, a fitter reads a database column name. This turns the
 * common shapes into Hungarian with the field's name as the form shows it; anything it does
 * not recognise is shown as sent, so nothing is ever hidden.
 */
private val FIELDS: Map<String, String> = mapOf(
    "title" to "Megnevezés",
    "name" to "Név",
    "full_name" to "Név",
    "partner_id" to "Partner",
    "contact_id" to "Kapcsolattartó",
    "project_type_id" to "Projekt típusa",
    "assigned_to" to "Felelős",
    "currency" to "Pénznem",
    "email" to "E-mail",
    "contact_email" to "E-mail",
    "to" to "Címzett",
    "phone" to "Telefon",
    "tax_number" to "Adószám",
    "eu_tax_number" to "Közösségi adószám",
    "country" to "Ország",
    "vehicle_plate" to "Rendszám",
    "vehicle_vin" to "Alvázszám",
    "vin" to "Alvázszám",
    "description" to "Leírás",
    "quantity" to "Mennyiség",
    "unit_price" to "Egységár",
    "what" to "Mire várunk",
    "due_date" to "Határidő",
    "mileage_in" to "Km-óra állás",
    "key_count" to "Kulcsok száma",
    "fuel_level" to "Üzemanyagszint",
    "note" to "Megjegyzés",
    "subject" to "Tárgy",
    "body" to "Szöveg",
    "kind" to "Típus",
    "start_date" to "Kezdet",
    "end_date" to "Vége",
    "odometer" to "Km-óra állás",
    "quoted_value_minor" to "Ajánlati összeg",
    "quote_valid_until" to "Ajánlat érvényessége",
)

private fun field(key: String): String = FIELDS[key] ?: key.replace('_', ' ')

private val RULES: List<Pair<Regex, (MatchResult) -> String>> = listOf(
    Regex("^([a-z_]+) is required$") to { m -> "${field(m.groupValues[1])}: kötelező kitölteni." },
    Regex("^([a-z_]+) is not a valid (?:email )?address$") to { m -> "${field(m.groupValues[1])}: nem érvényes e-mail cím." },
    Regex("^([a-z_]+) is too long \\(at most (\\d+) characters\\)$") to { m ->
        "${field(m.groupValues[1])}: legfeljebb ${m.groupValues[2]} karakter lehet."
    },
    Regex("^([a-z_]+) is at most (\\d+) characters$") to { m ->
        "${field(m.groupValues[1])}: legfeljebb ${m.groupValues[2]} karakter lehet."
    },
    Regex("^([a-z_]+) cannot be negative$") to { m -> "${field(m.groupValues[1])}: nem lehet negatív." },
    Regex("^([a-z_]+) is not a user$") to { m -> "${field(m.groupValues[1])}: nincs ilyen felhasználó." },
    Regex("^([a-z_]+) must be positive$") to { m -> "${field(m.groupValues[1])}: pozitív szám kell." },
    Regex("^country must be a two-letter ISO code$") to { _ -> "Ország: kétbetűs kód kell (pl. HU, DE)." },
    Regex("^choose or create a partner before converting the lead$") to { _ ->
        "Válassz vagy hozz létre partnert a megrendeléshez."
    },
    Regex("^email or phone is required$") to { _ -> "Adj meg e-mail címet vagy telefonszámot." },
    Regex("^vin: .*") to { _ -> "Alvázszám: 17 betű és szám, I, O és Q nélkül." },
    Regex("^end_date must not be before start_date$") to { _ -> "A vége nem lehet a kezdet előtt." },
    Regex("^the period contains no working days.*") to { _ -> "Az időszakban nincs munkanap (csak hétvége vagy ünnep)." },
    Regex("^this project type has no build specification$") to { _ -> "Ennek a projekttípusnak nincs műszaki specifikációja." },
    Regex("^the employee has left$") to { _ -> "A munkatárs már kilépett." },
)

/** A validation message in Hungarian; unknown shapes come back unchanged. */
fun huValidation(detail: String): String {
    val text = detail.trim()
    // Hungarian already (some rules answer in Hungarian): leave it.
    if (text.any { it in "áéíóöőúüűÁÉÍÓÖŐÚÜŰ" }) return text
    for ((regex, render) in RULES) {
        regex.matchEntire(text)?.let { return render(it) }
    }
    return text
}
