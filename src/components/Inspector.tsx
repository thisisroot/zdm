import { formatBytes, formatEta, formatSpeed } from '../lib/categories'
import { useConnectionSpeeds } from '../hooks/useConnectionSpeeds'
import type { DownloadRecord, SpeedUnit } from '../lib/types'
import { Sparkline } from './Sparkline'

interface InspectorProps {
  record: DownloadRecord | null
  queueName: string | null
  speedUnit: SpeedUnit
  speedHistory: number[]
  onPause: () => void
  onResume: () => void
  onRetry: () => void
  onOpenFolder: () => void
  onRemove: () => void
}

export function Inspector({ record, queueName, speedUnit, speedHistory, onPause, onResume, onRetry, onOpenFolder, onRemove }: InspectorProps) {
  const connSpeeds = useConnectionSpeeds(record)

  if (!record) {
    return (
      <aside className="inspector">
        <div className="empty">Select a download to inspect its connections.</div>
      </aside>
    )
  }

  const total = record.totalSize
  const rows = record.activeChunks
    .slice()
    .sort((a, b) => a.start - b.start)
    .map((c) => {
      const span = c.end - c.start + 1
      const pct = span > 0 ? Math.min(100, (c.bytesDone / span) * 100) : 0
      return { ...c, pct, speed: connSpeeds[c.start] ?? 0 }
    })

  return (
    <aside className="inspector">
      <div className="insp-head">
        <div className="fname">{record.name}</div>
        <div className="furl">{record.url}</div>
      </div>

      {record.error && <div className="error-box">{record.error}</div>}

      <div className="insp-stats">
        <div className="stat-box"><div className="k">Size</div><div className="v">{formatBytes(total)}</div></div>
        <div className="stat-box">
          <div className="k">Progress</div>
          <div className="v">{total ? Math.round((record.downloaded / total) * 100) : 0}%</div>
        </div>
        <div className="stat-box"><div className="k">Status</div><div className="v" style={{ textTransform: 'capitalize' }}>{record.status}</div></div>
        <div className="stat-box"><div className="k">ETA</div><div className="v">{formatEta(record)}</div></div>
        <div className="stat-box"><div className="k">Queue</div><div className="v">{queueName ?? '—'}</div></div>
        <div className="stat-box"><div className="k">Save folder</div><div className="v" style={{ fontSize: 11.5 }}>{record.destination}</div></div>
      </div>

      <div className="insp-section">
        <h4>Throughput</h4>
        <Sparkline data={speedHistory} width={608} height={128} />
      </div>

      <div className="insp-section">
        <h4>Connections ({rows.length || (record.status === 'downloading' ? 1 : 0)})</h4>
        {rows.length === 0 ? (
          <div style={{ color: 'var(--text-faint)', fontSize: 12 }}>
            {record.status === 'downloading'
              ? "Single connection — server doesn't support ranged requests."
              : 'No active connections.'}
          </div>
        ) : (
          <table className="conn-table">
            <thead>
              <tr><th>#</th><th>Byte range</th><th>Progress</th><th>{speedUnit === 'megabit' ? 'Mbps' : 'MB/s'}</th></tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={r.start}>
                  <td className="num">{i + 1}</td>
                  <td>{formatBytes(r.start)}–{formatBytes(r.end)}</td>
                  <td><div className="conn-mini"><i style={{ width: `${r.pct}%` }} /></div></td>
                  <td className="num">{formatSpeed(r.speed, speedUnit).replace(/ (MB\/s|Mbps)$/, '')}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="insp-actions">
        {record.status === 'downloading' && <button className="btn" onClick={onPause}>Pause</button>}
        {record.status === 'paused' && <button className="btn" onClick={onResume}>Resume</button>}
        {(record.status === 'failed' || record.status === 'canceled') && (
          <button className="btn" onClick={onRetry}>Retry</button>
        )}
        {record.status === 'completed' && <button className="btn" onClick={onOpenFolder}>Open folder</button>}
        <button className="btn insp-actions-danger" style={{ color: 'var(--error)' }} onClick={onRemove}>
          Remove
        </button>
      </div>
    </aside>
  )
}
