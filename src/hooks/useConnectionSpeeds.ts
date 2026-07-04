import { useEffect, useRef, useState } from 'react'
import type { DownloadRecord } from '../lib/types'

/** Derives a live MB/s-style rate per active connection by diffing successive
 * `bytesDone` snapshots — real measurement from real Progress events, not a
 * simulated readout. Keyed by each chunk's start offset, which is stable for
 * the lifetime of that chunk's transfer. */
export function useConnectionSpeeds(record: DownloadRecord | null): Record<number, number> {
  const prevRef = useRef<{ time: number; byStart: Record<number, number> }>({ time: Date.now(), byStart: {} })
  const [speeds, setSpeeds] = useState<Record<number, number>>({})

  useEffect(() => {
    if (!record) return
    const now = Date.now()
    const elapsed = (now - prevRef.current.time) / 1000
    if (elapsed <= 0.05) return

    const next: Record<number, number> = {}
    record.activeChunks.forEach((chunk) => {
      const prevBytes = prevRef.current.byStart[chunk.start] ?? chunk.bytesDone
      next[chunk.start] = Math.max(0, (chunk.bytesDone - prevBytes) / elapsed)
    })
    prevRef.current = { time: now, byStart: Object.fromEntries(record.activeChunks.map((c) => [c.start, c.bytesDone])) }
    setSpeeds(next)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [record?.activeChunks])

  return speeds
}
