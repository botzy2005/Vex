package com.vex.connect.core.link

import com.vex.connect.core.domain.model.DeviceLink
import com.vex.connect.core.domain.model.LinkNotice
import com.vex.connect.core.domain.model.LinkPhase
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.util.candidateAddresses
import com.vex.connect.core.domain.util.hexToBytes
import com.vex.connect.core.domain.util.reconnectDelayMs
import java.io.IOException
import java.net.SocketTimeoutException
import java.util.concurrent.ConcurrentHashMap
import javax.net.ssl.SSLException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vex_link_ffi.LinkFailure
import uniffi.vex_link_ffi.SessionClient
import uniffi.vex_link_ffi.SessionMessage
import uniffi.vex_link_ffi.fingerprint

/**
 * Keeps this phone connected to the PCs it has paired with, while the app is on screen.
 *
 * Stage 2 deliberately has no background service: when the app leaves the screen the links are closed politely (Bye) and
 * reopen when it comes back. A persistent background connection is a later, opt-in stage.
 *
 * For each paired PC one worker: find the PC (announced address first, last address second), open pinned TLS, run the
 * Noise session, then ping until the link drops, and retry with growing delays. The user can disconnect on purpose
 * ([disconnect], which holds the link down until [reconnect]) or remove the PC ([remove], which tells it).
 */
class LinkManager(
    private val scope: CoroutineScope,
    private val devices: DeviceRepository,
    private val identitySecret: () -> ByteArray,
    private val discovery: NsdDiscovery,
    private val phoneName: suspend () -> String,
    private val clockMillis: () -> Long = System::currentTimeMillis,
) {
    private val _links = MutableStateFlow<Map<String, DeviceLink>>(emptyMap())

    /** Link state per paired device id. A device with no entry is offline. */
    val links: StateFlow<Map<String, DeviceLink>> = _links.asStateFlow()

    private val _notice = MutableStateFlow<LinkNotice?>(null)
    val notice: StateFlow<LinkNotice?> = _notice.asStateFlow()

    private class Live(val client: SessionClient, val io: FrameIo)

    private val workers = ConcurrentHashMap<String, Job>()
    private val held = ConcurrentHashMap.newKeySet<String>()
    private val live = ConcurrentHashMap<String, Live>()
    @Volatile private var visible = false
    private val noticeCounter = java.util.concurrent.atomic.AtomicLong()

    init {
        scope.launch { devices.devices.collect { sync(it) } }
    }

    /** Called with true when the app comes to the screen and false when it leaves. */
    fun setVisible(on: Boolean) {
        visible = on
        if (on) {
            discovery.start()
            scope.launch { sync(devices.snapshot()) }
        } else {
            discovery.stop()
            workers.keys.toList().forEach { stopWorker(it, politely = true) }
            _links.update { cur -> cur.filterValues { l -> l.held } }
        }
    }

    fun disconnect(id: String) {
        held.add(id)
        stopWorker(id, politely = true)
        setLink(id, DeviceLink(LinkPhase.Offline, "Disconnected by you", held = true))
    }

    fun reconnect(id: String) {
        held.remove(id)
        setLink(id, DeviceLink(LinkPhase.Connecting))
        scope.launch { sync(devices.snapshot()) }
    }

    /** Forget this PC on the phone and, if it is connected right now, tell it to forget the phone too. */
    suspend fun remove(id: String) {
        withContext(Dispatchers.IO) {
            live[id]?.let { l ->
                runCatching { l.io.write(l.client.seal(SessionMessage.Revoked)) }
            }
        }
        held.remove(id)
        stopWorker(id, politely = false)
        devices.remove(id)
        _links.update { it - id }
    }

    suspend fun rename(id: String, name: String) {
        devices.rename(id, name)
    }

    fun dismissNotice(id: Long) {
        _notice.update { if (it?.id == id) null else it }
    }

    /** The short fingerprint for comparing with the PC's screen, or "" when the stored key is damaged. */
    fun fingerprintOf(device: PairedDevice): String =
        device.publicKeyHex.hexToBytes()?.let { runCatching { fingerprint(it) }.getOrNull() }.orEmpty()

    private fun sync(list: List<PairedDevice>) {
        val ids = list.map { it.id }.toSet()
        workers.keys.filter { it !in ids }.forEach { stopWorker(it, politely = false) }
        _links.update { cur -> cur.filterKeys { it in ids } }
        if (!visible) return
        for (d in list) {
            if (d.id in held || workers[d.id]?.isActive == true) continue
            workers[d.id] = scope.launch(Dispatchers.IO) { worker(d.id) }
        }
    }

    /** Network work never runs on the calling (often main) thread: the polite goodbye and the close happen on IO. */
    private fun stopWorker(id: String, politely: Boolean) {
        val job = workers.remove(id)
        val l = live.remove(id)
        scope.launch(Dispatchers.IO) {
            if (politely && l != null) runCatching { l.io.write(l.client.seal(SessionMessage.Bye)) }
            job?.cancel()
            l?.io?.close()
        }
    }

    private fun setLink(id: String, link: DeviceLink) {
        _links.update { it + (id to link) }
    }

    private sealed interface Outcome {
        data class Ended(val reason: String?, val wasConnected: Boolean) : Outcome
        data object RemovedByPc : Outcome
    }

    private suspend fun worker(id: String) {
        var failures = 0
        var first = true
        while (currentCoroutineContext().isActive) {
            val device = devices.get(id) ?: return
            setLink(id, DeviceLink(LinkPhase.Connecting))
            val outcome = try {
                session(device, waitForAnnouncement = first)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Outcome.Ended(describe(e), wasConnected = false)
            }
            first = false
            when (outcome) {
                Outcome.RemovedByPc -> {
                    devices.remove(id)
                    _links.update { it - id }
                    _notice.value = LinkNotice(noticeCounter.incrementAndGet(), "${device.name} removed this phone. Pair again to reconnect.")
                    return
                }
                is Outcome.Ended -> {
                    failures = if (outcome.wasConnected) 0 else failures + 1
                    setLink(id, DeviceLink(LinkPhase.Offline, outcome.reason))
                }
            }
            delay(reconnectDelayMs(failures))
        }
    }

    private fun describe(e: Throwable): String = when (e) {
        is SocketTimeoutException -> "The PC stopped answering"
        is SSLException -> "The PC's security certificate does not match. Remove it and pair again."
        is LinkFailure -> "Secure channel error"
        is IOException -> "PC not reachable on this network"
        else -> "Could not connect"
    }

    private suspend fun session(device: PairedDevice, waitForAnnouncement: Boolean): Outcome = withContext<Outcome>(Dispatchers.IO) {
        val pcPublic = device.publicKeyHex.hexToBytes() ?: return@withContext Outcome.Ended("Saved PC data is damaged. Remove it and pair again.", false)
        val pin = device.tlsFingerprintHex.hexToBytes() ?: return@withContext Outcome.Ended("Saved PC data is damaged. Remove it and pair again.", false)

        var nearby = discovery.found.value.firstOrNull { it.id == device.id }
        if (nearby == null && waitForAnnouncement) {
            nearby = withTimeoutOrNull(2_500) { discovery.found.first { list -> list.any { it.id == device.id } } }?.firstOrNull { it.id == device.id }
        }
        val candidates = candidateAddresses(nearby, device.lastAddress, device.lastPort, PinnedTls.DEFAULT_PORT)
        if (candidates.isEmpty()) return@withContext Outcome.Ended("PC not found on this network", false)

        var socket: javax.net.ssl.SSLSocket? = null
        var chosen: Pair<String, Int>? = null
        var security = false
        for (c in candidates) {
            try {
                socket = PinnedTls.connect(c.first, c.second, pin, readTimeoutMs = 45_000)
                chosen = c
                break
            } catch (e: SSLException) {
                security = true
            } catch (e: IOException) {
                // next candidate
            }
        }
        if (socket == null || chosen == null) {
            return@withContext Outcome.Ended(
                if (security) "The PC's security certificate does not match. Remove it and pair again." else "PC not reachable on this network",
                false,
            )
        }
        val io = FrameIo(socket)
        val client = SessionClient(identitySecret(), pcPublic)
        val job = currentCoroutineContext()[Job]
        val closer = job?.invokeOnCompletion { if (it != null) io.close() }
        var mine: Live? = null
        try {
            io.write(client.firstMessage())
            val reply = io.read()
                ?: return@withContext Outcome.Ended("This PC does not recognise this phone. Remove it here and pair again.", false)
            runCatching { io.write(client.finish(reply, phoneName())) }
            val second = io.read() ?: return@withContext Outcome.Ended("The PC closed the connection", false)
            when (val m = client.open(second)) {
                is SessionMessage.Hello -> {}
                is SessionMessage.Revoked -> return@withContext Outcome.RemovedByPc
                else -> return@withContext Outcome.Ended("The PC sent an unexpected message", false)
            }

            val l = Live(client, io)
            mine = l
            live[device.id] = l
            devices.touch(device.id, clockMillis(), chosen.first, chosen.second)
            setLink(device.id, DeviceLink(LinkPhase.Connected))

            coroutineScope<Outcome> {
                val pinger = launch {
                    var n = 0UL
                    while (isActive) {
                        delay(15_000)
                        n += 1UL
                        runCatching { io.write(client.seal(SessionMessage.Ping(n))) }
                    }
                }
                val ended: Outcome = try {
                    var result: Outcome? = null
                    while (result == null) {
                        val frame = io.read()
                        result = if (frame == null) {
                            Outcome.Ended("Disconnected", true)
                        } else {
                            when (val m = client.open(frame)) {
                                is SessionMessage.Ping -> { io.write(client.seal(SessionMessage.Pong(m.n))); null }
                                is SessionMessage.Pong -> null
                                is SessionMessage.Bye -> Outcome.Ended("The PC disconnected", true)
                                is SessionMessage.Revoked -> Outcome.RemovedByPc
                                is SessionMessage.Error -> Outcome.Ended("The PC reported an error", true)
                                is SessionMessage.Hello -> Outcome.Ended("Unexpected message from the PC", true)
                            }
                        }
                    }
                    checkNotNull(result)
                } finally {
                    pinger.cancel()
                }
                ended
            }
        } finally {
            closer?.dispose()
            mine?.let { live.remove(device.id, it) }
            io.close()
            runCatching { client.close() }
        }
    }
}
