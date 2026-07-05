export interface AccentVars {
  accent: string
  accentStrong: string
  accentInk: string
  accentSoft: string
}

export interface AccentTheme {
  id: string
  label: string
  swatch: string
  dark: AccentVars
  light: AccentVars
}

// Rose stands in for "red" — a true error-red would be indistinguishable from
// the app's own error state, so this is a red family that reads as a deliberate
// accent rather than a warning. Green is deliberately excluded: the progress
// bar is intentionally a different color from the accent, and offering green
// as an accent choice would collapse that distinction.
export const ACCENT_THEMES: AccentTheme[] = [
  {
    id: 'amber',
    label: 'Amber',
    swatch: '#e8a33d',
    dark: { accent: '#e8a33d', accentStrong: '#f7bd63', accentInk: '#2b1c05', accentSoft: 'rgba(232,163,61,.14)' },
    light: { accent: '#b06a17', accentStrong: '#8f5510', accentInk: '#fffaf0', accentSoft: 'rgba(176,106,23,.10)' },
  },
  {
    id: 'teal',
    label: 'Teal',
    swatch: '#4fa89c',
    dark: { accent: '#4fa89c', accentStrong: '#72c9bd', accentInk: '#04211d', accentSoft: 'rgba(79,168,156,.16)' },
    light: { accent: '#2c7a70', accentStrong: '#1f5c54', accentInk: '#f2fbfa', accentSoft: 'rgba(44,122,112,.10)' },
  },
  {
    id: 'blue',
    label: 'Blue',
    swatch: '#5b9bd8',
    dark: { accent: '#5b9bd8', accentStrong: '#7fb4e6', accentInk: '#04182b', accentSoft: 'rgba(91,155,216,.16)' },
    light: { accent: '#2f6fae', accentStrong: '#235587', accentInk: '#f2f8fd', accentSoft: 'rgba(47,111,174,.10)' },
  },
  {
    id: 'purple',
    label: 'Purple',
    swatch: '#a586d1',
    dark: { accent: '#a586d1', accentStrong: '#c0a8e6', accentInk: '#1c1230', accentSoft: 'rgba(165,134,209,.16)' },
    light: { accent: '#7a58a8', accentStrong: '#5f4282', accentInk: '#faf7fd', accentSoft: 'rgba(122,88,168,.10)' },
  },
  {
    id: 'rose',
    label: 'Rose',
    swatch: '#d1789a',
    dark: { accent: '#d1789a', accentStrong: '#e79bb6', accentInk: '#2b0f18', accentSoft: 'rgba(209,120,154,.16)' },
    light: { accent: '#a8496b', accentStrong: '#833650', accentInk: '#fdf5f7', accentSoft: 'rgba(168,73,107,.10)' },
  },
]

export function accentThemeById(id: string): AccentTheme {
  return ACCENT_THEMES.find((t) => t.id === id) ?? ACCENT_THEMES[0]
}

/** Sets the accent CSS custom properties for whichever light/dark mode is
 * currently active — call again after toggling theme, since the two modes
 * use different values for the same accent. */
export function applyAccent(id: string) {
  const theme = accentThemeById(id)
  const root = document.documentElement
  const explicitTheme = root.getAttribute('data-theme')
  const isDark = explicitTheme ? explicitTheme === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches
  const vars = isDark ? theme.dark : theme.light
  root.style.setProperty('--accent', vars.accent)
  root.style.setProperty('--accent-strong', vars.accentStrong)
  root.style.setProperty('--accent-ink', vars.accentInk)
  root.style.setProperty('--accent-soft', vars.accentSoft)
}
