import { Sparkline } from './Sparkline'
import { PauseIcon, PlayIcon, PlusIcon, SearchIcon, SettingsIcon, ThemeIcon } from './icons'
import type { SpeedUnit } from '../lib/types'

interface TopBarProps {
  totalSpeedBps: number
  speedUnit: SpeedUnit
  speedHistory: number[]
  activeCount: number
  activeConnections: number
  queuedCount: number
  pausedCount: number
  search: string
  onSearchChange: (value: string) => void
  onToggleTheme: () => void
  onToggleAllActive: () => void
  onOpenSettings: () => void
  onOpenAdd: () => void
}

export function TopBar({
  totalSpeedBps,
  speedUnit,
  speedHistory,
  activeCount,
  activeConnections,
  queuedCount,
  pausedCount,
  search,
  onSearchChange,
  onToggleTheme,
  onToggleAllActive,
  onOpenSettings,
  onOpenAdd,
}: TopBarProps) {
  // Queued counts as "running" for this control too — toggle_all holds
  // not-yet-started downloads too, so the icon needs to reflect that.
  const anyRunnable = activeCount > 0 || queuedCount > 0
  const isMegabit = speedUnit === 'megabit'
  const figure = isMegabit ? (totalSpeedBps * 8) / 1e6 : totalSpeedBps / 1e6
  return (
    <div className="topbar">
      <div className="hero-speed">
        <span className="figure tabular">{figure.toFixed(1)}</span>
        <span className="unit">{isMegabit ? 'Mbps' : 'MB/s'}</span>
      </div>
      <Sparkline className="spark" data={speedHistory} width={240} height={68} />
      <div className="hero-meta">
        <span className="label">Active</span>
        <span className="value tabular">
          {activeCount} downloads · {activeConnections} connections
        </span>
      </div>
      <div style={{ flex: 1 }} />
      <div className="search">
        <SearchIcon />
        <input type="text" placeholder="Filter downloads…" value={search} onChange={(e) => onSearchChange(e.target.value)} />
      </div>
      <button className="btn btn-icon" title="Toggle theme" aria-label="Toggle theme" onClick={onToggleTheme}>
        <ThemeIcon />
      </button>
      {(anyRunnable || pausedCount > 0) && (
        <button
          className="btn btn-icon"
          title={anyRunnable ? 'Pause all active downloads' : 'Resume all paused downloads'}
          aria-label={anyRunnable ? 'Pause all active downloads' : 'Resume all paused downloads'}
          onClick={onToggleAllActive}
        >
          {anyRunnable ? <PauseIcon /> : <PlayIcon />}
        </button>
      )}
      <button className="btn btn-icon" title="Settings" aria-label="Settings" onClick={onOpenSettings}>
        <SettingsIcon />
      </button>
      <button className="btn btn-primary" onClick={onOpenAdd}>
        <PlusIcon />
        Add Download
      </button>
    </div>
  )
}
