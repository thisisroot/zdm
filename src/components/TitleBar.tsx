import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import logoMark from '../assets/logo-mark.png'
import { WinCloseIcon, WinMaximizeIcon, WinMinimizeIcon, WinRestoreIcon } from './icons'

const appWindow = getCurrentWindow()

export function TitleBar() {
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    appWindow.isMaximized().then(setMaximized)
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized)
    })
    return () => {
      unlisten.then((f) => f())
    }
  }, [])

  return (
    <div className="titlebar">
      <div className="titlebar-drag" data-tauri-drag-region onDoubleClick={() => appWindow.toggleMaximize()}>
        <img src={logoMark} alt="" width={18} height={18} className="brand-mark" />
        <span>ZDM</span>
      </div>
      <div className="titlebar-controls">
        <button title="Minimize" aria-label="Minimize" onClick={() => appWindow.minimize()}>
          <WinMinimizeIcon />
        </button>
        <button title={maximized ? 'Restore' : 'Maximize'} aria-label="Maximize" onClick={() => appWindow.toggleMaximize()}>
          {maximized ? <WinRestoreIcon /> : <WinMaximizeIcon />}
        </button>
        <button title="Close" aria-label="Close" className="titlebar-close" onClick={() => appWindow.close()}>
          <WinCloseIcon />
        </button>
      </div>
    </div>
  )
}
