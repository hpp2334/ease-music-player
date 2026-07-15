package com.kutedev.easemusicplayer.utils

import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

class Item(val data: ByteArray, var remaining: Int)

class ByteQueue {
    private val queue = ConcurrentLinkedQueue<Item>()
    private val lock = ReentrantLock()

    fun put(bytes: ByteArray) {
        lock.withLock {
            queue.add(Item(bytes, bytes.size))
        }
    }

    fun isEmpty(): Boolean {
        lock.withLock {
            return queue.isEmpty()
        }
    }

    fun take(n: Int): ByteArray {
        lock.withLock {
            val result = ByteArray(n)
            var resultOffset = 0

            var takeRemaining = n
            while (takeRemaining > 0 && queue.isNotEmpty()) {
                val item = queue.peek()!!

                val bytesToTake = minOf(takeRemaining, item.remaining)
                item.data.copyInto(result, resultOffset, item.data.size - item.remaining, item.data.size - item.remaining + bytesToTake)
                resultOffset += bytesToTake
                takeRemaining -= bytesToTake
                item.remaining -= bytesToTake

                if (item.remaining == 0) {
                    queue.poll()
                }
            }
            return result.copyOf(resultOffset)
        }
    }

    fun clear() {
        lock.withLock {
            queue.clear()
        }
    }
}