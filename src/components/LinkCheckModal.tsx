import { formatBytes } from '../lib/categories'
import type { UrlProbe } from '../lib/api'

interface LinkCheckModalProps {
  open: boolean
  mode: 'single' | 'batch'
  probes: UrlProbe[]
  busy: boolean
  onCancel: () => void
  onConfirm: () => void
}

export function LinkCheckModal({ open, mode, probes, busy, onCancel, onConfirm }: LinkCheckModalProps) {
  if (!open) return null

  const valid = probes.filter((p) => !p.error)
  const broken = probes.filter((p) => p.error)
  const totalSize = valid.reduce((sum, p) => sum + (p.totalSize ?? 0), 0)
  const hasUnknownSizes = valid.some((p) => p.totalSize == null)
  const allBroken = mode === 'batch' && valid.length === 0 && broken.length > 0

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="modal" style={{ width: 460 }}>
        <h2>{broken.length > 0 ? 'Some links are unreachable' : mode === 'batch' ? 'Confirm batch download' : 'Confirm download'}</h2>
        <div className="sub">
          {mode === 'batch'
            ? `${valid.length} of ${probes.length} link${probes.length === 1 ? '' : 's'} checked out.`
            : broken.length > 0
              ? 'This link didn’t respond the way a download normally would.'
              : 'This link is reachable.'}
        </div>

        <div className="modal-body">
          <div className="insp-stats">
            <div className="stat-box">
              <div className="k">{mode === 'batch' ? 'Valid files' : 'Status'}</div>
              <div className="v">{mode === 'batch' ? valid.length : broken.length > 0 ? 'Unreachable' : 'Reachable'}</div>
            </div>
            <div className="stat-box">
              <div className="k">Total size</div>
              <div className="v">{valid.length > 0 ? `${hasUnknownSizes ? '≥ ' : ''}${formatBytes(totalSize || null)}` : '—'}</div>
            </div>
          </div>

          {broken.length > 0 && (
            <div className="batch-preview" style={{ marginTop: 14 }}>
              {broken.slice(0, 6).map((p) => (
                <div className="brow" key={p.url}>
                  <span className="f">{p.url}</span>
                  <span style={{ color: 'var(--error)', fontSize: 10.5, flex: 'none' }}>{p.error}</span>
                </div>
              ))}
              {broken.length > 6 && <div className="more">+ {broken.length - 6} more…</div>}
            </div>
          )}
        </div>

        <div className="modal-actions">
          <button className="btn" onClick={onCancel} disabled={busy}>Cancel</button>
          <button className="btn btn-primary" onClick={onConfirm} disabled={busy || allBroken}>
            {broken.length > 0 ? 'Download Anyway' : mode === 'batch' ? `Queue ${valid.length} Downloads` : 'Start Download'}
          </button>
        </div>
      </div>
    </div>
  )
}
