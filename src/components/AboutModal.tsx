import { useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import logoMark from '../assets/logo-mark.png'

interface AboutModalProps {
  open: boolean
  onClose: () => void
}

const REPO = 'thisisroot/zdm'

type UpdateStatus =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'up-to-date' }
  | { kind: 'available'; version: string; url: string }
  | { kind: 'no-releases' }
  | { kind: 'error' }

export function AboutModal({ open, onClose }: AboutModalProps) {
  const [version, setVersion] = useState('')
  const [status, setStatus] = useState<UpdateStatus>({ kind: 'idle' })

  useEffect(() => {
    if (!open) return
    setStatus({ kind: 'idle' })
    getVersion().then(setVersion)
  }, [open])

  if (!open) return null

  async function checkForUpdates() {
    setStatus({ kind: 'checking' })
    try {
      const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
      if (res.status === 404) {
        setStatus({ kind: 'no-releases' })
        return
      }
      if (!res.ok) throw new Error(`GitHub API returned ${res.status}`)
      const data = await res.json()
      const latest = String(data.tag_name ?? '').replace(/^v/, '')
      if (latest && isNewer(latest, version)) {
        setStatus({ kind: 'available', version: latest, url: data.html_url })
      } else {
        setStatus({ kind: 'up-to-date' })
      }
    } catch {
      setStatus({ kind: 'error' })
    }
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" style={{ width: 380 }}>
        <div className="about-head">
          <img src={logoMark} alt="" width={56} height={56} />
          <div>
            <h2 style={{ margin: 0 }}>ZDM</h2>
            <div className="sub" style={{ margin: 0 }}>Version {version || '—'}</div>
          </div>
        </div>
        <p className="about-desc">
          A fast, segmented download manager built with Tauri and Rust. Open source under the MIT license.
        </p>
        <div className="about-links">
          <button className="btn btn-sm" onClick={() => openUrl(`https://github.com/${REPO}`)}>View on GitHub</button>
          <button className="btn btn-sm" onClick={() => openUrl(`https://github.com/${REPO}/blob/main/LICENSE`)}>License</button>
        </div>
        <div className="about-update">
          <button className="btn btn-sm" onClick={checkForUpdates} disabled={status.kind === 'checking'}>
            {status.kind === 'checking' ? 'Checking…' : 'Check for Updates'}
          </button>
          {status.kind === 'up-to-date' && <span className="about-status ok">You’re up to date.</span>}
          {status.kind === 'no-releases' && <span className="about-status">No published releases yet.</span>}
          {status.kind === 'error' && <span className="about-status error">Couldn’t check for updates.</span>}
          {status.kind === 'available' && (
            <span className="about-status">
              v{status.version} is available —{' '}
              <button className="link-btn" onClick={() => openUrl(status.url)}>view release</button>
            </span>
          )}
        </div>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  )
}

function isNewer(latest: string, current: string): boolean {
  const a = latest.split('.').map(Number)
  const b = current.split('.').map(Number)
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const na = a[i] ?? 0
    const nb = b[i] ?? 0
    if (na !== nb) return na > nb
  }
  return false
}
