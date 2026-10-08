package hu.autotherm.autocrm

import hu.autotherm.autocrm.ui.common.huValidation
import org.junit.Assert.assertEquals
import org.junit.Test

/** The server's English validation lines reach the fitter in Hungarian. */
class ValidationTextTest {
    @Test
    fun commonShapesAreTranslated() {
        assertEquals("Partner: kötelező kitölteni.", huValidation("partner_id is required"))
        assertEquals("E-mail: nem érvényes e-mail cím.", huValidation("email is not a valid address"))
        assertEquals("Megnevezés: legfeljebb 200 karakter lehet.", huValidation("title is at most 200 characters"))
    }

    @Test
    fun unknownTextIsShownAsSent() {
        assertEquals("something new", huValidation("something new"))
        assertEquals("Hibás a jelenlegi jelszó.", huValidation("Hibás a jelenlegi jelszó."))
    }
}
