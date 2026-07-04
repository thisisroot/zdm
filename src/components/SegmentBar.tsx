import type { CSSProperties } from 'react'
import type { DownloadRecord } from '../lib/types'

// A tight ramp around the progress hue — distinct connections read as
// "parts of one whole," not an arbitrary rainbow.
const SEG_RAMP = ['#4fb477', '#46a76c', '#5cc086', '#3e9860', '#69c491', '#48ab70', '#57b47d', '#3aa066']

interface SegmentBarProps {
  record: DownloadRecord
}

/**
 * Renders real per-connection telemetry: a faint wash shows the aggregate
 * fraction already downloaded, and a crisp block per currently in-flight
 * chunk shows exactly where that connection is and how far it's gotten —
 * straight from `active_chunks` on the last Progress event, not a simulation.
 */
export function SegmentBar({ record }: SegmentBarProps) {
  const total = record.totalSize
  const overallFraction = total ? Math.min(1, record.downloaded / total) : 0

  if (!total || record.activeChunks.length === 0) {
    return (
      <div className="segbar">
        <i>
          <span className="fill" style={{ '--p': `${overallFraction * 100}%` } as CSSProperties} />
        </i>
      </div>
    )
  }

  return (
    <div className="segbar segbar-multi">
      <div className="segbar-base" style={{ width: `${overallFraction * 100}%` }} />
      {record.activeChunks.map((chunk, i) => {
        const span = chunk.end - chunk.start + 1
        const left = (chunk.start / total) * 100
        const width = Math.max((span / total) * 100, 0.4)
        const fillPct = span > 0 ? Math.min(100, (chunk.bytesDone / span) * 100) : 0
        return (
          <div key={chunk.start} className="segbar-chunk" style={{ left: `${left}%`, width: `${width}%` }}>
            <div className="segbar-chunk-fill" style={{ width: `${fillPct}%`, background: SEG_RAMP[i % SEG_RAMP.length] }} />
          </div>
        )
      })}
    </div>
  )
}
