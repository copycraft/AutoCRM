// How the app looks for a person: follow the device, or always light / always dark. Stored
// on the server with their preferences (`theme`, 0066), picked first on the invitation
// page; this browser keeps a copy so a reload paints in the right colours at once.

export type Theme = 'system' | 'light' | 'dark';

export const THEMES: Theme[] = ['system', 'light', 'dark'];

const KEY = 'autocrm:theme';

export function isTheme(value: unknown): value is Theme {
  return value === 'system' || value === 'light' || value === 'dark';
}

/** Sets the look now (globals.css reads `data-theme`) and remembers it on this device. */
export function applyTheme(theme: Theme): void {
  if (typeof document === 'undefined') return;
  document.documentElement.dataset.theme = theme;
  try {
    window.localStorage.setItem(KEY, theme);
  } catch {
    /* private mode: the server copy still applies after sign-in */
  }
}

/**
 * Inline in <head>, before anything paints: the look this browser last used. Without it a
 * dark-theme user would see a white flash on every reload until preferences arrive.
 */
export const THEME_BOOT_SCRIPT = `(function(){var t='system';try{var s=localStorage.getItem('${KEY}');if(s==='light'||s==='dark'||s==='system')t=s}catch(e){}document.documentElement.dataset.theme=t})();`;
