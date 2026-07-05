import { useEffect, useState } from 'react'
import { CATEGORIES } from '../lib/categories'
import { api } from '../lib/api'
import { ACCENT_THEMES } from '../lib/accents'
import type { Settings } from '../lib/types'
import { DirectoryField } from './DirectoryField'
import { AboutModal } from './AboutModal'

interface SettingsModalProps {
  open: boolean
  settings: Settings
  onClose: () => void
  onSave: (settings: Settings) => Promise<void>
}

export function SettingsModal({ open, settings, onClose, onSave }: SettingsModalProps) {
  const [draft, setDraft] = useState<Settings>(settings)
  const [aboutOpen, setAboutOpen] = useState(false)

  useEffect(() => {
    if (open) setDraft(settings)
  }, [open, settings])

  if (!open) return null

  function setCategoryDir(id: string, dir: string) {
    setDraft((prev) => ({ ...prev, categoryDirs: { ...prev.categoryDirs, [id]: dir } }))
  }

  async function done() {
    await onSave(draft)
    onClose()
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" style={{ width: 460 }}>
        <h2>Settings</h2>
        <div className="sub">Tune how aggressively ZDM pulls bandwidth, and where files land.</div>
        <div className="modal-body">
          <div className="settings-grid">
            <div className="setting-row">
              <div>
                <div className="t">Max simultaneous downloads</div>
                <div className="d">How many files transfer at once</div>
              </div>
              <RangeWithValue
                min={1}
                max={10}
                value={draft.maxSimultaneousDownloads}
                onChange={(v) => setDraft((p) => ({ ...p, maxSimultaneousDownloads: v }))}
              />
            </div>
            <div className="setting-row">
              <div>
                <div className="t">Default connections per download</div>
                <div className="d">Segments a new download splits into</div>
              </div>
              <RangeWithValue
                min={1}
                max={16}
                value={draft.defaultConnections}
                onChange={(v) => setDraft((p) => ({ ...p, defaultConnections: v }))}
              />
            </div>
            <div className="setting-row">
              <div>
                <div className="t">Notify on completion</div>
                <div className="d">System notification when a file finishes</div>
              </div>
              <div
                className={`switch${draft.notifyOnCompletion ? ' on' : ''}`}
                onClick={() => setDraft((p) => ({ ...p, notifyOnCompletion: !p.notifyOnCompletion }))}
              >
                <i />
              </div>
            </div>
            <div className="setting-row">
              <div>
                <div className="t">Theme color</div>
                <div className="d">Accent used for buttons, highlights, and progress</div>
              </div>
              <div className="accent-swatches">
                {ACCENT_THEMES.map((theme) => (
                  <button
                    key={theme.id}
                    type="button"
                    className={`accent-swatch${draft.accentColor === theme.id ? ' on' : ''}`}
                    style={{ background: theme.swatch }}
                    title={theme.label}
                    aria-label={theme.label}
                    onClick={() => setDraft((p) => ({ ...p, accentColor: theme.id }))}
                  />
                ))}
              </div>
            </div>

            <hr className="settings-divider" />

            <DirectoryField
              label="Default download folder"
              value={draft.defaultDir}
              onChange={(v) => setDraft((p) => ({ ...p, defaultDir: v }))}
            />

            <div>
              <div className="t" style={{ marginBottom: 8 }}>Category folders</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {CATEGORIES.map((cat) => (
                  <CategoryDirRow
                    key={cat.id}
                    label={cat.label}
                    value={draft.categoryDirs[cat.id] ?? ''}
                    onChange={(v) => setCategoryDir(cat.id, v)}
                  />
                ))}
              </div>
            </div>
          </div>
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={() => setAboutOpen(true)} style={{ marginRight: 'auto' }}>About ZDM</button>
          <button className="btn btn-primary" onClick={done}>Done</button>
        </div>
      </div>

      <AboutModal open={aboutOpen} onClose={() => setAboutOpen(false)} />
    </div>
  )
}

function RangeWithValue({ min, max, value, onChange }: { min: number; max: number; value: number; onChange: (v: number) => void }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
      <input type="range" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value))} />
      <span className="tabular" style={{ fontFamily: 'var(--font-mono)', fontSize: 12, width: 20, textAlign: 'right' }}>{value}</span>
    </div>
  )
}

function CategoryDirRow({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  async function browse() {
    const picked = await api.chooseDirectory()
    if (picked) onChange(picked)
  }
  return (
    <div className="dir-row">
      <span className="lbl">{label}</span>
      <div className="with-btn">
        <input type="text" value={value} onChange={(e) => onChange(e.target.value)} />
        <button type="button" className="btn btn-sm" onClick={browse}>Browse…</button>
      </div>
    </div>
  )
}
