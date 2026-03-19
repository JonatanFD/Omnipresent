package com.omnipresent.network

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.SocketTimeoutException

data class DiscoveredServer(val ip: String, val port: Int, val token: Int)

class DiscoveryClient {
    suspend fun discoverServer(timeoutMs: Int = 2500): DiscoveredServer? {
        return withContext(Dispatchers.IO) {
            var socket: DatagramSocket? = null
            try {
                socket = DatagramSocket().apply {
                    broadcast = true
                    soTimeout = timeoutMs
                }

                val sendData = "OMNIPRESENT_DISCOVERY".toByteArray()
                val sendPacket = DatagramPacket(
                    sendData, sendData.size,
                    InetAddress.getByName("255.255.255.255"), 9091
                )

                socket.send(sendPacket)

                val recvBuf = ByteArray(1024)
                val recvPacket = DatagramPacket(recvBuf, recvBuf.size)
                
                Log.d("UDP_DISCOVERY", "Waiting for broadcast response...")
                socket.receive(recvPacket)

                val response = String(recvPacket.data, 0, recvPacket.length)
                Log.d("UDP_DISCOVERY", "Received response: $response")

                if (response.startsWith("OMNIPRESENT_HERE")) {
                    val parts = response.split("|")
                    if (parts.size >= 3) {
                        val serverIp = recvPacket.address.hostAddress ?: return@withContext null
                        val serverPort = parts[1].toInt()
                        val token = parts[2].toInt()
                        return@withContext DiscoveredServer(serverIp, serverPort, token)
                    }
                }
            } catch (e: SocketTimeoutException) {
                Log.d("UDP_DISCOVERY", "Timeout waiting for discovery response.")
            } catch (e: Exception) {
                Log.e("UDP_DISCOVERY", "Server not found: ${e.message}")
            } finally {
                socket?.close()
            }
            null
        }
    }
}
