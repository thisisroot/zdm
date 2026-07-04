import { LogoIcon } from './icons'

export function TitleBar() {
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="dots">
        <span className="dot" /><span className="dot" /><span className="dot" />
      </div>
      <div className="brand">
        <LogoIcon />
        <span>ZDM</span>
      </div>
    </div>
  )
}
