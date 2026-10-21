import { createRoot } from 'react-dom/client';
import { App } from './App';
import { Overlay } from './overlay/Overlay';
import { Viewer } from './viewer/Viewer';
import { RecordingCountdown } from './recording/Recording';
import { Splash } from './Splash';
import './styles.css';
import { native, type Settings } from './native';

const params = new URLSearchParams(location.search);
const surface = params.get('countdown') === '1' ? 'countdown' : params.get('splash') === '1' ? 'splash' : params.get('overlay') === '1' ? 'overlay' : params.get('viewer') === '1' ? 'viewer' : 'shelf';
document.documentElement.dataset.surface = surface;
const applyTheme = (theme: Settings['theme']) => {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem('theme', theme);
};
applyTheme((localStorage.getItem('theme') as Settings['theme']) ?? 'system');
if (surface !== 'overlay' && surface !== 'splash' && surface !== 'countdown') {
  native.onSettingsChanged(settings => applyTheme(settings.theme))
    .then(() => native.settings()).then(settings => applyTheme(settings.theme)).catch(() => {});
}

// The browser menu never shows; text fields keep the native one for copy/paste and spelling.
document.addEventListener('contextmenu', e => {
  const target = e.target as HTMLElement | null;
  if (!target?.closest('input:not([type=button]):not([type=checkbox]):not([type=radio]), textarea, [contenteditable=""], [contenteditable="true"]')) e.preventDefault();
});

// No StrictMode: the shelf must subscribe once and send a single ready signal.
createRoot(document.getElementById('root')!).render(surface === 'countdown' ? <RecordingCountdown /> : surface === 'splash' ? <Splash /> : surface === 'overlay' ? <Overlay /> : surface === 'viewer' ? <Viewer /> : <App />);
