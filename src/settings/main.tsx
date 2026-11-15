import { createRoot } from 'react-dom/client';
import { createBackend } from '../backend';
import { BackendContext } from '../backend/hooks';
import { SettingsApp } from './App';
import '../styles/face.css';
import './settings.css';

createBackend()
  .then((backend) => {
    document.documentElement.dataset.backend = backend.kind;
    createRoot(document.getElementById('root')!).render(
      <BackendContext.Provider value={backend}>
        <SettingsApp />
      </BackendContext.Provider>,
    );
  })
  .catch((e) => {
    document.body.textContent = String(e);
  });
