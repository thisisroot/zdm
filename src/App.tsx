import { useEffect, useMemo, useState } from 'react'
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener'
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
import { CancelIcon, PauseIcon, PlayIcon } from './components/icons'

export default function App() {
  const { downloads, queues, settings, loaded, refreshQueues, updateSettings } = useDownloads()
  const [filter, setFilter] = useState<Filter>({ kind: 'all' })
  const [search, setSearch] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set())
  const [dragId, setDragId] = useState<string | null>(null)
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

  // checkedIds can outlive its row (e.g. after a removal) — always derive the
  // working set from what's actually still in `list` rather than trusting the set directly.
  const checkedList = useMemo(() => list.filter((d) => checkedIds.has(d.id)), [list, checkedIds])
  const allChecked = list.length > 0 && checkedList.length === list.length

  function toggleChecked(id: string) {
    setCheckedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  function toggleCheckAll() {
    setCheckedIds(allChecked ? new Set() : new Set(list.map((d) => d.id)))
  }

  // Reordering resequences the entire download set, so it only makes sense
  // against the unfiltered, unsearched view where "the list" and "everything" are the same thing.
  const canReorder = filter.kind === 'all' && !search.trim()

  function handleDrop(targetId: string) {
    if (!dragId || dragId === targetId) return
    const ids = list.map((d) => d.id)
    const from = ids.indexOf(dragId)
    const to = ids.indexOf(targetId)
    if (from === -1 || to === -1) return
    ids.splice(to, 0, ids.splice(from, 1)[0])
    api.reorderDownloads(ids)
    setDragId(null)
  }

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

  async function openFile(record: DownloadRecord) {
    await openPath(record.destination)
  }

  // Anything short of a finished download has no standalone value as a partial
  // file, so removing it also cleans up the partial bytes it already wrote —
  // only a completed download's file is worth preserving on removal.
  async function removeRecord(id: string) {
    const record = downloads[id]
    const deleteFile = record ? record.status !== 'completed' : false
    await api.removeDownload(id, deleteFile)
    if (selectedId === id) setSelectedId(null)
    setCheckedIds((prev) => {
      if (!prev.has(id)) return prev
      const next = new Set(prev)
      next.delete(id)
      return next
    })
  }

  async function bulkPause() {
    await Promise.all(checkedList.filter((d) => d.status === 'downloading').map((d) => api.pauseDownload(d.id)))
  }

  async function bulkResume() {
    await Promise.all(checkedList.filter((d) => d.status === 'paused').map((d) => api.resumeDownload(d.id)))
  }

  async function bulkRemove() {
    await Promise.all(checkedList.map((d) => removeRecord(d.id)))
  }

  const finishedList = useMemo(() => list.filter((d) => d.status === 'completed' || d.status === 'failed' || d.status === 'canceled'), [list])

  // Unlike per-row remove, "delete files" here is an explicit, all-or-nothing
  // choice the user made for this action — it applies even to completed downloads.
  async function clearFinished(deleteFiles: boolean) {
    if (finishedList.length === 0) return
    if (deleteFiles && !window.confirm(`Remove ${finishedList.length} download(s) from the list and delete their files from disk?`)) return
    const ids = finishedList.map((d) => d.id)
    await Promise.all(ids.map((id) => api.removeDownload(id, deleteFiles)))
    setCheckedIds((prev) => {
      const next = new Set(prev)
      ids.forEach((id) => next.delete(id))
      return next
    })
    if (selectedId && ids.includes(selectedId)) setSelectedId(null)
  }

  async function deleteQueue(queueId: string) {
    await api.deleteQueue(queueId)
    await refreshQueues()
    if (filter.kind === 'queue' && filter.queueId === queueId) setFilter({ kind: 'all' })
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
        <Rail
          downloads={allSorted}
          queues={queues}
          filter={filter}
          onFilterChange={setFilter}
          onToggleQueue={toggleQueue}
          onDeleteQueue={deleteQueue}
        />

        <main className="list-pane">
          {list.length > 0 && (
            <div className="list-toolbar">
              <label className="check-all">
                <input type="checkbox" checked={allChecked} onChange={toggleCheckAll} />
                {checkedList.length > 0 ? `${checkedList.length} selected` : 'Select all'}
              </label>
              <div className="toolbar-right">
                {checkedList.length > 0 && (
                  <div className="bulk-actions">
                    <button className="btn btn-sm" onClick={bulkPause}><PauseIcon />Pause</button>
                    <button className="btn btn-sm" onClick={bulkResume}><PlayIcon />Resume</button>
                    <button className="btn btn-sm btn-danger" onClick={bulkRemove}><CancelIcon />Remove</button>
                  </div>
                )}
                {finishedList.length > 0 && (
                  <div className="bulk-actions">
                    <button className="btn btn-sm" onClick={() => clearFinished(false)}>Clear finished</button>
                    <button className="btn btn-sm btn-danger" onClick={() => clearFinished(true)}>Clear + delete files</button>
                  </div>
                )}
              </div>
            </div>
          )}
          {list.length === 0 ? (
            <div className="list-empty">No downloads match this filter.</div>
          ) : (
            list.map((record) => (
              <div
                key={record.id}
                className={`row-drag${dragId === record.id ? ' dragging' : ''}`}
                draggable={canReorder}
                onDragStart={() => setDragId(record.id)}
                onDragEnd={() => setDragId(null)}
                onDragOver={(e) => canReorder && e.preventDefault()}
                onDrop={() => canReorder && handleDrop(record.id)}
              >
                <DownloadRow
                  record={record}
                  selected={record.id === selectedId}
                  checked={checkedIds.has(record.id)}
                  queueName={queueNameFor(record)}
                  onSelect={() => setSelectedId(record.id)}
                  onToggleCheck={() => toggleChecked(record.id)}
                  onPause={() => api.pauseDownload(record.id)}
                  onResume={() => api.resumeDownload(record.id)}
                  onRetry={() => api.retryDownload(record.id)}
                  onRemove={() => removeRecord(record.id)}
                  onOpenFolder={() => openFolder(record)}
                  onOpenFile={() => openFile(record)}
                />
              </div>
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
