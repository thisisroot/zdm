import type { ReactElement, SVGProps } from 'react'

const base: SVGProps<SVGSVGElement> = { viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }

export const PauseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" /></svg>
)
export const PlayIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><path d="M7 4l13 8-13 8z" /></svg>
)
export const RetryIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><path d="M21 12a9 9 0 1 1-3-6.7M21 3v6h-6" /></svg>
)
export const CancelIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><path d="M6 6l12 12M18 6 6 18" /></svg>
)
export const FolderIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><path d="M3 7h6l2 2h10v10H3z" /></svg>
)
export const SearchIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" /></svg>
)
export const ThemeIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4m11.4-11.4 1.4-1.4" />
  </svg>
)
export const SettingsIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 0 1-4 0v-.09A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 0 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.55 1H21a2 2 0 0 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1Z" />
  </svg>
)
export const PlusIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} strokeWidth={2.2} {...p}><path d="M12 5v14M5 12h14" /></svg>
)
export const ListIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} strokeWidth={1.8} {...p}><path d="M4 6h16M4 12h10M4 18h13" /></svg>
)
export const LogoIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}><path d="M12 3v12m0 0-4-4m4 4 4-4M4 19h16" /></svg>
)

export const CategoryIcons: Record<string, (p: SVGProps<SVGSVGElement>) => ReactElement> = {
  disc: (p) => <svg {...base} strokeWidth={1.8} {...p}><circle cx="12" cy="12" r="9" /><circle cx="12" cy="12" r="2.4" /></svg>,
  video: (p) => <svg {...base} strokeWidth={1.8} {...p}><rect x="3" y="5" width="14" height="14" rx="2" /><path d="m21 8-4 3 4 3z" /></svg>,
  audio: (p) => (
    <svg {...base} strokeWidth={1.8} {...p}>
      <path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
    </svg>
  ),
  archive: (p) => (
    <svg {...base} strokeWidth={1.8} {...p}>
      <rect x="4" y="3" width="16" height="18" rx="2" /><path d="M9 3v18M9 7h2M9 11h2M9 15h2" />
    </svg>
  ),
  docs: (p) => <svg {...base} strokeWidth={1.8} {...p}><path d="M7 3h7l5 5v13H7z" /><path d="M14 3v5h5" /></svg>,
  software: (p) => <svg {...base} strokeWidth={1.8} {...p}><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 9h18" /></svg>,
  image: (p) => (
    <svg {...base} strokeWidth={1.8} {...p}>
      <rect x="3" y="4" width="18" height="16" rx="2" /><circle cx="9" cy="10" r="1.6" /><path d="m5 17 4.5-5 3.5 4 2.5-3 4.5 4" />
    </svg>
  ),
}
