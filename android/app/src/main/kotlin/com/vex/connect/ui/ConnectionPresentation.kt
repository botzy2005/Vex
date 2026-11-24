package com.vex.connect.ui

import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.domain.model.ConnectionState

/** How a [ConnectionState] is shown. Pure, so every state is unit-tested even though only one can occur in Stage 1. */
data class ConnectionPresentation(
    val pill: String,
    val title: String,
    val tone: Tone,
    /** Only a connection attempt in progress may animate. */
    val pulsing: Boolean,
)

fun presentConnection(state: ConnectionState): ConnectionPresentation = when (state) {
    ConnectionState.NotPaired -> ConnectionPresentation("Not connected", "No PC paired yet", Tone.Warn, pulsing = false)
    is ConnectionState.Offline -> ConnectionPresentation("Offline", "${state.deviceName} is not reachable", Tone.Neutral, pulsing = false)
    is ConnectionState.Connecting -> ConnectionPresentation("Connecting", "Connecting to ${state.deviceName}", Tone.Accent, pulsing = true)
    is ConnectionState.Connected -> ConnectionPresentation("Connected", "Connected to ${state.deviceName}", Tone.Ok, pulsing = false)
}
