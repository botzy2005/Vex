import { createRoot } from 'react-dom/client';
import { createBackend } from './backend';
import { BackendContext } from './backend/hooks';
import { IslandApp } from './island/App';
import './styles/island.css';
import './styles/face.css';

const root = createRoot(document.getElementById('root')!);

createBackend()
  .then((backend) => {
    document.documentElement.dataset.backend = backend.kind;
    root.render(
      <BackendContext.Provider value={backend}>
        <IslandApp />
      </BackendContext.Provider>,
    );
  })
  .catch((e) => {
    root.render(<pre style={{ color: '#fff', background: '#400', padding: 12 }}>{String(e)}</pre>);
  });
