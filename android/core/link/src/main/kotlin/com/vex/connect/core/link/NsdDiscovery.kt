package com.vex.connect.core.link

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import com.vex.connect.core.domain.model.NearbyPc
import java.net.Inet4Address
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Finds VEX Desktop on the local network (mDNS / DNS-SD, service `_vexconnect._tcp`).
 *
 * Discovery only tells the phone where a PC is. It never pairs and never trusts: the PC's name here is just a label, and
 * every connection is still checked against the keys and certificate saved at pairing. Runs only between [start] and [stop].
 */
class NsdDiscovery(context: Context, private val onError: (Throwable) -> Unit = {}) {
    private val nsd = context.applicationContext.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val byInstance = LinkedHashMap<String, NearbyPc>()
    private val pending = ArrayDeque<NsdServiceInfo>()
    private var resolving = false
    private var listener: NsdManager.DiscoveryListener? = null
    private val _found = MutableStateFlow<List<NearbyPc>>(emptyList())

    /** PCs currently announcing themselves, one entry per device id. */
    val found: StateFlow<List<NearbyPc>> = _found.asStateFlow()

    @Synchronized
    fun start() {
        if (listener != null) return
        val l = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                synchronized(this@NsdDiscovery) { listener = null }
            }

            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}

            override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                guarded {
                    synchronized(this@NsdDiscovery) {
                        pending.addLast(serviceInfo)
                        pump()
                    }
                }
            }

            override fun onServiceLost(serviceInfo: NsdServiceInfo) {
                guarded {
                    synchronized(this@NsdDiscovery) {
                        byInstance.remove(serviceInfo.serviceName)
                        publish()
                    }
                }
            }
        }
        listener = l
        try {
            nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, l)
        } catch (e: Exception) {
            listener = null
        }
    }

    @Synchronized
    fun stop() {
        listener?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        listener = null
        pending.clear()
        resolving = false
        byInstance.clear()
        publish()
    }

    /** Android resolves one service at a time, so found services queue up. */
    @Suppress("DEPRECATION")
    private fun pump() {
        if (resolving) return
        val next = pending.removeFirstOrNull() ?: return
        resolving = true
        try {
            nsd.resolveService(
                next,
                object : NsdManager.ResolveListener {
                    override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
                        guarded {
                            synchronized(this@NsdDiscovery) {
                                resolving = false
                                pump()
                            }
                        }
                    }

                    override fun onServiceResolved(serviceInfo: NsdServiceInfo) {
                        guarded {
                            synchronized(this@NsdDiscovery) {
                                try {
                                    resolved(serviceInfo)
                                } finally {
                                    resolving = false
                                }
                                pump()
                            }
                        }
                    }
                },
            )
        } catch (e: Exception) {
            resolving = false
        }
    }

    @Suppress("DEPRECATION")
    private fun resolved(info: NsdServiceInfo) {
        val host = info.host as? Inet4Address ?: return // the PC listens on IPv4 only
        val attrs = info.attributes
        val id = attrs["id"]?.toString(Charsets.UTF_8)?.takeIf { it.length == 32 } ?: return
        val name = attrs["n"]?.toString(Charsets.UTF_8)?.take(40)?.ifBlank { null } ?: info.serviceName
        byInstance[info.serviceName] = NearbyPc(id = id, name = name, host = host.hostAddress ?: return, port = info.port)
        publish()
    }

    /**
     * Android calls these listeners on its own thread, where an exception would end the whole app. A bad announcement
     * from the network must never do that: it is reported (Settings, Diagnostics) and the listing carries on.
     */
    private inline fun guarded(block: () -> Unit) {
        try {
            block()
        } catch (e: Exception) {
            onError(e)
        }
    }

    private fun publish() {
        _found.value = byInstance.values.distinctBy { it.id }
    }

    companion object {
        const val SERVICE_TYPE = "_vexconnect._tcp."
    }
}
