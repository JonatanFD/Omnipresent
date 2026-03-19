package com.omnipresent.network

import com.omnipresent.protocol.TrackpadMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.SocketTimeoutException

class UdpClient(private val ip: String, private val port: Int) {
    private var socket: DatagramSocket? = null
    private val address: InetAddress by lazy { InetAddress.getByName(ip) }

    suspend fun send(message: TrackpadMessage) = withContext(Dispatchers.IO) {
        try {
            if (socket == null || socket?.isClosed == true) {
                socket = DatagramSocket()
            }
            val bytes = message.toByteArray()
            val packet = DatagramPacket(bytes, bytes.size, address, port)
            socket?.send(packet)
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }

    suspend fun receive(): String? = withContext(Dispatchers.IO) {
        try {
            if (socket == null || socket?.isClosed == true) {
                socket = DatagramSocket()
            }
            // Set timeout so we don't block forever
            socket?.soTimeout = 1000
            val buffer = ByteArray(1024)
            val packet = DatagramPacket(buffer, buffer.size)
            socket?.receive(packet)
            String(packet.data, 0, packet.length)
        } catch (e: SocketTimeoutException) {
            null
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }

    fun close() {
        socket?.close()
        socket = null
    }
}
