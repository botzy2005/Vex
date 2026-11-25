package com.vex.connect.core.domain.model

/** A VEX Desktop found on the local network (not necessarily paired). */
data class NearbyPc(
    /** Device id announced by the PC (32 hex characters). */
    val id: String,
    val name: String,
    val host: String,
    val port: Int,
)

/** What the pairing screen shows. Exactly one of these at a time. */
sealed interface PairingUi {
    data object Idle : PairingUi
    data object Connecting : PairingUi

    /** Both screens show [code]; the user says whether they match. */
    data class Confirm(val code: String, val pcName: String) : PairingUi

    /** This phone's user said yes; waiting for the PC's user. */
    data class WaitingForPc(val code: String, val pcName: String) : PairingUi
    data class Done(val pcName: String) : PairingUi
    data class Declined(val byPhone: Boolean) : PairingUi
    data class Failed(val message: String) : PairingUi
}

enum class LinkPhase { Offline, Connecting, Connected }

/**
 * The live link to one paired PC. [detail] is a short human reason when [phase] is [LinkPhase.Offline]
 * (for example "PC not found on this network"); [held] means the user disconnected on purpose, so nothing reconnects
 * until they press Reconnect.
 */
data class DeviceLink(
    val phase: LinkPhase = LinkPhase.Offline,
    val detail: String? = null,
    val held: Boolean = false,
)

/** One-off message for the person (shown once): the PC removed this phone, and so on. */
data class LinkNotice(val id: Long, val text: String)
