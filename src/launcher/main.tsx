import { createRoot } from 'react-dom/client';
import { createBackend } from '../backend';
import { BackendContext } from '../backend/hooks';
import { LauncherApp } from './LauncherApp';
import '../styles/tokens.css';
import './launcher.css';

createBackend()
  .then((backend) => {
    document.documentElement.dataset.backend = backend.kind;
    createRoot(document.getElementById('root')!).render(
      <BackendContext.Provider value={backend}>
        <LauncherApp />
      </BackendContext.Provider>,
    );
  })
  .catch((e) => {
    document.body.textContent = String(e);
  });
