import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { api } from '../lib/api'
import type { DownloadRecord, QueueInfo, Settings } from '../lib/types'

/** Central store: hydrates from the backend once, then patches in place as
 * `download-updated` events arrive — no polling. */
export function useDownloads() {
  const [downloads, setDownloads] = useState<Record<string, DownloadRecord>>({})
  const [queues, setQueues] = useState<QueueInfo[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    let unlistenUpdated: (() => void) | undefined
    let unlistenRemoved: (() => void) | undefined
    let cancelled = false

    ;(async () => {
      const [list, qs, st] = await Promise.all([api.listDownloads(), api.listQueues(), api.getSettings()])
      if (cancelled) return
      setDownloads(Object.fromEntries(list.map((d) => [d.id, d])))
      setQueues(qs)
      setSettings(st)
      setLoaded(true)

      unlistenUpdated = await listen<DownloadRecord>('download-updated', (event) => {
        const record = event.payload
        setDownloads((prev) => ({ ...prev, [record.id]: record }))
      })
      unlistenRemoved = await listen<string>('download-removed', (event) => {
        const id = event.payload
        setDownloads((prev) => {
          const next = { ...prev }
          delete next[id]
          return next
        })
      })
    })()

    return () => {
      cancelled = true
      unlistenUpdated?.()
      unlistenRemoved?.()
    }
  }, [])

  const refreshQueues = useCallback(async () => {
    setQueues(await api.listQueues())
  }, [])

  const updateSettings = useCallback(async (next: Settings) => {
    setSettings(next)
    await api.updateSettings(next)
  }, [])

  return { downloads, queues, settings, loaded, refreshQueues, updateSettings }
}
