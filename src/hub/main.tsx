import { createRoot } from 'react-dom/client';
import { createBackend } from '../backend';
import { BackendContext } from '../backend/hooks';
import { HubApp } from './HubApp';
import '../styles/tokens.css';
import './hub.css';

createBackend()
  .then((backend) => {
    document.documentElement.dataset.backend = backend.kind;
    createRoot(document.getElementById('root')!).render(
      <BackendContext.Provider value={backend}>
        <HubApp />
      </BackendContext.Provider>,
    );
  })
  .catch((e) => {
    document.body.textContent = String(e);
  });
