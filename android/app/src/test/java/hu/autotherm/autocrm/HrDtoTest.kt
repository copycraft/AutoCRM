package hu.autotherm.autocrm

import hu.autotherm.autocrm.data.api.Employee
import hu.autotherm.autocrm.data.api.SessionUser
import hu.autotherm.autocrm.data.api.StaffUser
import hu.autotherm.autocrm.data.auth.SessionStore
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** What the HR and users screens rely on: the wire shapes and who gets which drawer entry. */
class HrDtoTest {

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `an older server without hr_access parses as no access`() {
        val user = json.decodeFromString(
            SessionUser.serializer(),
            """{"id":1,"email":"a@b.hu","display_name":"A","role":"office","must_change_password":false,"session_kind":"mobile"}""",
        )
        assertFalse(user.hrAccess)
    }

    @Test
    fun `an employee parses with and without optional fields`() {
        val full = json.decodeFromString(
            Employee.serializer(),
            """{"id":7,"full_name":"Kiss Péter","email":"p@a.hu","company_phone":"+36 30 1","personal_phone":"+36 20 2",
               "photo_url":"https://s3/x.jpg","archived_at":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}""",
        )
        assertEquals("Kiss Péter", full.fullName)
        assertEquals("+36 20 2", full.personalPhone)
        assertNull(full.archivedAt)

        val bare = json.decodeFromString(Employee.serializer(), """{"id":8,"full_name":"Nagy Anna"}""")
        assertNull(bare.email)
        assertNull(bare.photoUrl)
    }

    @Test
    fun `a staff user carries the hr flag`() {
        val u = json.decodeFromString(
            StaffUser.serializer(),
            """{"id":2,"email":"a@b.hu","display_name":"A","role":"office","is_active":true,"must_change_password":false,"hr_access":true}""",
        )
        assertTrue(u.hrAccess)
    }

    private fun account(role: String, hr: Boolean) = SessionStore.Account(
        userId = 1, email = "a@b.hu", displayName = "A", role = role,
        mustChangePassword = false, hrAccess = hr,
    )

    @Test
    fun `the users page is for admins only, whatever the hr flag`() {
        assertTrue(account("admin", true).isAdmin)
        assertFalse(account("office", true).isAdmin)
        assertFalse(account("viewer", false).isAdmin)
    }
}
