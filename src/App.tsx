import { useEffect, useMemo, useState } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { TitleBar } from './components/TitleBar'
import { TopBar } from './components/TopBar'
import { Rail } from './components/Rail'
import { DownloadRow } from './components/DownloadRow'
import { Inspector } from './components/Inspector'
import { AddDownloadModal } from './components/AddDownloadModal'
import { SettingsModal } from './components/SettingsModal'
import { useDownloads } from './hooks/useDownloads'
import { useHistory } from './hooks/useHistory'
import { api } from './lib/api'
import { matchesFilter, type DownloadRecord, type Filter } from './lib/types'

export default function App() {
  const { downloads, queues, settings, loaded, refreshQueues, updateSettings } = useDownloads()
  const [filter, setFilter] = useState<Filter>({ kind: 'all' })
  const [search, setSearch] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)

  const allSorted = useMemo(() => Object.values(downloads).sort((a, b) => a.seq - b.seq), [downloads])

  useEffect(() => {
    if (selectedId && downloads[selectedId]) return
    if (allSorted.length) setSelectedId(allSorted[0].id)
  }, [allSorted, selectedId, downloads])

  const list = useMemo(() => {
    const filtered = allSorted.filter((d) => matchesFilter(d, filter))
    const q = search.trim().toLowerCase()
    return q ? filtered.filter((d) => d.name.toLowerCase().includes(q)) : filtered
  }, [allSorted, filter, search])

  const activeDownloads = useMemo(() => allSorted.filter((d) => d.status === 'downloading'), [allSorted])
  const totalSpeed = activeDownloads.reduce((sum, d) => sum + d.speedBps, 0)
  const totalConnections = activeDownloads.reduce((sum, d) => sum + d.connections, 0)
  const heroHistory = useHistory(() => totalSpeed)

  const selectedRecord = selectedId ? downloads[selectedId] ?? null : null
  const selectedHistory = useHistory(() => selectedRecord?.speedBps ?? 0)
  const selectedQueueName = selectedRecord ? queues.find((q) => q.id === selectedRecord.queue)?.name ?? selectedRecord.queue : null

  function queueNameFor(record: DownloadRecord): string | null {
    return queues.find((q) => q.id === record.queue)?.name ?? record.queue
  }

  async function toggleQueue(queueId: string) {
    const members = allSorted.filter((d) => d.queue === queueId)
    const anyRunning = members.some((d) => d.status === 'downloading')
    await Promise.all(
      members.map((d) => {
        if (anyRunning && d.status === 'downloading') return api.pauseDownload(d.id)
        if (!anyRunning && d.status === 'paused') return api.resumeDownload(d.id)
        return Promise.resolve()
      }),
    )
  }

  async function openFolder(record: DownloadRecord) {
    await revealItemInDir(record.destination)
  }

  async function removeRecord(id: string) {
    await api.removeDownload(id, false)
    if (selectedId === id) setSelectedId(null)
  }

  if (!loaded || !settings) {
    return (
      <div className="app" style={{ alignItems: 'center', justifyContent: 'center', color: 'var(--text-faint)' }}>
        Loading ZDM…
      </div>
    )
  }

  return (
    <div className="app">
      <TitleBar />
      <TopBar
        totalSpeedBps={totalSpeed}
        speedHistory={heroHistory}
        activeCount={activeDownloads.length}
        activeConnections={totalConnections}
        search={search}
        onSearchChange={setSearch}
        onToggleTheme={() => {
          const root = document.documentElement
          const current = root.getAttribute('data-theme') || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
          root.setAttribute('data-theme', current === 'dark' ? 'light' : 'dark')
        }}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAdd={() => setAddOpen(true)}
      />

      <div className="body">
        <Rail downloads={allSorted} queues={queues} filter={filter} onFilterChange={setFilter} onToggleQueue={toggleQueue} />

        <main className="list-pane">
          {list.length === 0 ? (
            <div className="list-empty">No downloads match this filter.</div>
          ) : (
            list.map((record) => (
              <DownloadRow
                key={record.id}
                record={record}
                selected={record.id === selectedId}
                queueName={queueNameFor(record)}
                onSelect={() => setSelectedId(record.id)}
                onPause={() => api.pauseDownload(record.id)}
                onResume={() => api.resumeDownload(record.id)}
                onRetry={() => api.retryDownload(record.id)}
                onCancel={() => api.cancelDownload(record.id)}
                onOpenFolder={() => openFolder(record)}
              />
            ))
          )}
        </main>

        <Inspector
          record={selectedRecord}
          queueName={selectedQueueName}
          speedHistory={selectedHistory}
          onPause={() => selectedRecord && api.pauseDownload(selectedRecord.id)}
          onResume={() => selectedRecord && api.resumeDownload(selectedRecord.id)}
          onRetry={() => selectedRecord && api.retryDownload(selectedRecord.id)}
          onOpenFolder={() => selectedRecord && openFolder(selectedRecord)}
          onRemove={() => selectedRecord && removeRecord(selectedRecord.id)}
        />
      </div>

      <AddDownloadModal
        open={addOpen}
        settings={settings}
        queues={queues}
        onClose={() => setAddOpen(false)}
        onAddSingle={async (args) => {
          const id = await api.addDownload(args)
          await refreshQueues()
          setSelectedId(id)
        }}
        onAddBatch={async (args) => {
          const ids = await api.addBatch(args)
          await refreshQueues()
          if (ids[0]) setSelectedId(ids[0])
        }}
      />

      <SettingsModal open={settingsOpen} settings={settings} onClose={() => setSettingsOpen(false)} onSave={updateSettings} />
    </div>
  )
}
