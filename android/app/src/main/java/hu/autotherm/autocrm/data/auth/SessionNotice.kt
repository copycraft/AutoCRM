package hu.autotherm.autocrm.data.auth

import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Set when the server ended the session (a 401 on a signed-in request), so the login screen
 * can say why it appeared instead of the app seeming to have logged itself out. A sign-out
 * the person asked for leaves it unset.
 */
object SessionNotice {
    val expired = MutableStateFlow(false)
}
