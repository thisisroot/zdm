import { useEffect, useState } from 'react'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import { CATEGORIES, detectCategory, filenameFromUrl, parseBatchPatternPreview } from '../lib/categories'
import type { QueueInfo, Settings } from '../lib/types'
import { DirectoryField } from './DirectoryField'

interface AddDownloadModalProps {
  open: boolean
  settings: Settings
  queues: QueueInfo[]
  onClose: () => void
  onAddSingle: (args: { url: string; destinationDir: string; connections: number; category: string; queue: string }) => Promise<void>
  onAddBatch: (args: {
    urlPattern: string
    destinationDir: string
    connections: number
    category: string
    queueName: string
  }) => Promise<void>
}

type Mode = 'single' | 'batch'

function isDownloadableUrl(value: string): boolean {
  try {
    const parsed = new URL(value)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
  } catch {
    return false
  }
}

export function AddDownloadModal({ open, settings, queues, onClose, onAddSingle, onAddBatch }: AddDownloadModalProps) {
  const [mode, setMode] = useState<Mode>('single')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const [url, setUrl] = useState('')
  const [saveDir, setSaveDir] = useState(settings.defaultDir)
  const [connections, setConnections] = useState(settings.defaultConnections)
  const [category, setCategory] = useState('')
  const [categoryTouched, setCategoryTouched] = useState(false)
  const [queueId, setQueueId] = useState(queues[0]?.id ?? 'default')

  const [batchUrl, setBatchUrl] = useState('')
  const [batchSaveDir, setBatchSaveDir] = useState(settings.defaultDir)
  const [batchConnections, setBatchConnections] = useState(settings.defaultConnections)
  const [batchCategory, setBatchCategory] = useState('')
  const [batchQueueName, setBatchQueueName] = useState('')
  const [batchQueueTouched, setBatchQueueTouched] = useState(false)

  // Reset to a blank slate every time the modal opens, then try to prefill
  // the URL from the clipboard — only if it actually looks like a download
  // link. Anything else (or an empty/unreadable clipboard) leaves the field
  // blank rather than showing a fake-looking example value.
  useEffect(() => {
    if (!open) return
    setError(null)
    setBusy(false)
    setCategoryTouched(false)
    setBatchQueueTouched(false)
    setCategory('')
    setBatchCategory('')
    setSaveDir(settings.defaultDir)
    setBatchSaveDir(settings.defaultDir)
    setBatchQueueName('')
    setUrl('')
    setBatchUrl('')

    let cancelled = false
    readText()
      .then((text) => {
        if (cancelled) return
        const trimmed = text.trim()
        if (isDownloadableUrl(trimmed)) {
          setUrl(trimmed)
          setBatchUrl(trimmed)
        }
      })
      .catch(() => {
        // No clipboard access (permission denied, empty clipboard, etc.) —
        // leaving the fields blank is the correct fallback either way.
      })
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  useEffect(() => {
    if (categoryTouched || !url.trim()) return
    const detected = detectCategory(filenameFromUrl(url))
    setCategory(detected)
    setSaveDir(settings.categoryDirs[detected] ?? settings.defaultDir)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url])

  const batchUrls = parseBatchPatternPreview(batchUrl)
  useEffect(() => {
    if (batchUrls.length === 0) return
    const detected = detectCategory(filenameFromUrl(batchUrls[0]))
    setBatchCategory(detected)
    setBatchSaveDir(settings.categoryDirs[detected] ?? settings.defaultDir)
    if (!batchQueueTouched) setBatchQueueName(batchUrl.split('/').pop() ?? batchUrl)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [batchUrl])

  if (!open) return null

  async function submit() {
    setError(null)
    if (mode === 'single' && !url.trim()) {
      setError('Enter a URL to download')
      return
    }
    if (mode === 'batch' && batchUrls.length === 0) {
      setError('Add a range like [01-99] to the URL pattern above')
      return
    }

    setBusy(true)
    try {
      if (mode === 'single') {
        let queue = queueId
        if (queueId === '__new__') queue = `New Queue ${queues.length + 1}`
        await onAddSingle({ url, destinationDir: saveDir, connections, category: category || 'other', queue })
      } else {
        await onAddBatch({
          urlPattern: batchUrl,
          destinationDir: batchSaveDir,
          connections: batchConnections,
          category: batchCategory || 'other',
          queueName: batchQueueName || batchUrl,
        })
      }
      onClose()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal">
        <h2>Add Download</h2>
        <div className="sub">ZDM will fetch this in parallel connections and reassemble it on completion.</div>

        <div className="seg-tabs">
          <button className={`seg-tab${mode === 'single' ? ' on' : ''}`} onClick={() => setMode('single')}>Single URL</button>
          <button className={`seg-tab${mode === 'batch' ? ' on' : ''}`} onClick={() => setMode('batch')}>Batch Pattern</button>
        </div>

        <div className="modal-body">
          {mode === 'single' ? (
            <>
              <div className="field">
                <label>URL</label>
                <input type="text" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://example.com/file.zip" autoFocus />
              </div>
              <DirectoryField label="Download folder" value={saveDir} onChange={setSaveDir} />
              <div className="row2">
                <div className="field">
                  <label>Queue</label>
                  <select value={queueId} onChange={(e) => setQueueId(e.target.value)}>
                    {queues.map((q) => (
                      <option key={q.id} value={q.id}>{q.name}</option>
                    ))}
                    <option value="__new__">+ New queue…</option>
                  </select>
                </div>
                <div className="field" style={{ maxWidth: 130 }}>
                  <label>Connections</label>
                  <Stepper value={connections} onChange={setConnections} />
                </div>
              </div>
              <div className="field">
                <label>Category</label>
                <div className="chips">
                  {CATEGORIES.map((c) => (
                    <span
                      key={c.id}
                      className={`chip${category === c.id ? ' on' : ''}`}
                      onClick={() => {
                        setCategoryTouched(true)
                        setCategory(c.id)
                        setSaveDir(settings.categoryDirs[c.id] ?? settings.defaultDir)
                      }}
                    >
                      {c.label}
                    </span>
                  ))}
                </div>
                <div className="hint">Auto-detected from the file extension — click to override.</div>
              </div>
            </>
          ) : (
            <>
              <div className="field">
                <label>URL pattern</label>
                <input type="text" value={batchUrl} onChange={(e) => setBatchUrl(e.target.value)} placeholder="https://example.com/part[01-99].zip" autoFocus />
                <div className="hint">
                  Wrap a numbered range in brackets — <code>part[01-99].zip</code> generates 99 files with matching zero-padding.
                </div>
              </div>
              <DirectoryField label="Download folder" value={batchSaveDir} onChange={setBatchSaveDir} />
              <div className="row2">
                <div className="field">
                  <label>Queue name</label>
                  <input
                    type="text"
                    value={batchQueueName}
                    onChange={(e) => {
                      setBatchQueueTouched(true)
                      setBatchQueueName(e.target.value)
                    }}
                    placeholder="My batch queue"
                  />
                </div>
                <div className="field" style={{ maxWidth: 150 }}>
                  <label>Conns / file</label>
                  <Stepper value={batchConnections} onChange={setBatchConnections} />
                </div>
              </div>
              <div className="field">
                <label>Category</label>
                <div className="chips">
                  {CATEGORIES.map((c) => (
                    <span
                      key={c.id}
                      className={`chip${batchCategory === c.id ? ' on' : ''}`}
                      onClick={() => {
                        setBatchCategory(c.id)
                        setBatchSaveDir(settings.categoryDirs[c.id] ?? settings.defaultDir)
                      }}
                    >
                      {c.label}
                    </span>
                  ))}
                </div>
              </div>
              <div className="field">
                <div className="batch-count"><b>{batchUrls.length}</b> files will be added to this queue</div>
                <div className="batch-preview">
                  {batchUrls.length === 0 ? (
                    <div className="hint" style={{ margin: 0 }}>Add a range like [01-99] to the URL above to preview generated files.</div>
                  ) : (
                    <>
                      {batchUrls.slice(0, 6).map((u) => (
                        <div className="brow" key={u}><span className="f">{filenameFromUrl(u)}</span></div>
                      ))}
                      {batchUrls.length > 6 && <div className="more">+ {batchUrls.length - 6} more…</div>}
                    </>
                  )}
                </div>
              </div>
            </>
          )}
        </div>

        <div className="modal-actions">
          {error && <span className="modal-error">{error}</span>}
          <button className="btn" onClick={onClose} disabled={busy}>Cancel</button>
          <button className="btn btn-primary" onClick={submit} disabled={busy}>
            {mode === 'batch' ? `Queue ${batchUrls.length || ''} Downloads` : 'Start Download'}
          </button>
        </div>
      </div>
    </div>
  )
}

function Stepper({ value, onChange }: { value: number; onChange: (n: number) => void }) {
  return (
    <div className="stepper">
      <button type="button" onClick={() => onChange(Math.max(1, value - 1))}>−</button>
      <span className="n tabular">{value}</span>
      <button type="button" onClick={() => onChange(Math.min(16, value + 1))}>+</button>
    </div>
  )
}
