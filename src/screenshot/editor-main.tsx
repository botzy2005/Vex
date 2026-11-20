import { createRoot } from 'react-dom/client';
import { createBackend } from '../backend';
import { BackendContext } from '../backend/hooks';
import { EditorApp } from './Editor';
import '../styles/tokens.css';
import './editor.css';

createBackend()
  .then((backend) => {
    document.documentElement.dataset.backend = backend.kind;
    createRoot(document.getElementById('root')!).render(
      <BackendContext.Provider value={backend}>
        <EditorApp />
      </BackendContext.Provider>,
    );
  })
  .catch((e) => {
    document.body.textContent = String(e);
  });
