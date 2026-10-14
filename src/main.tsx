import { createRoot } from 'react-dom/client';
import { App } from './App';
import { Overlay } from './overlay/Overlay';
import './styles.css';

const overlay = new URLSearchParams(location.search).get('overlay') === '1';
document.documentElement.dataset.surface = overlay ? 'overlay' : 'shelf';

// No StrictMode: the shelf must subscribe once and send a single ready signal.
createRoot(document.getElementById('root')!).render(overlay ? <Overlay /> : <App />);
