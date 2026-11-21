import { Component, lazy, Suspense, useState, type ComponentType, type LazyExoticComponent, type ReactNode } from 'react';
import { entryOf, type WidgetProps } from './registry';

const cache = new Map<string, LazyExoticComponent<ComponentType<WidgetProps>>>();

/** One lazy component per (kind, attempt); a new attempt re-runs the import after a failure. */
function lazyFor(kind: string, attempt: number) {
  const key = `${kind}#${attempt}`;
  let c = cache.get(key);
  if (!c) {
    const entry = entryOf(kind);
    c = lazy(entry ? entry.load : () => Promise.reject(new Error('This widget is not available in this version.')));
    cache.set(key, c);
  }
  return c;
}

interface BoundaryProps {
  onRemove(): void;
  onRetry(): void;
  children: ReactNode;
}

/** A widget that throws takes only its own card down. */
class Boundary extends Component<BoundaryProps, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error) {
    console.error('widget failed:', error);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="widget-error" role="alert" data-testid="widget-error">
        <div className="we-title">This widget stopped working.</div>
        <div className="we-detail">{this.state.error.message}</div>
        <div className="we-actions">
          <button className="hub-btn" onClick={this.props.onRetry}>Reload</button>
          <button className="hub-btn" onClick={this.props.onRemove}>Remove</button>
        </div>
      </div>
    );
  }
}

function Skeleton() {
  return <div className="widget-skeleton" data-testid="widget-loading" aria-label="Loading" />;
}

/** Lifecycle owner for one card body: lazy load, mount, error isolation. */
export function WidgetHost({ kind, size, active, onRemove }: WidgetProps & { onRemove(): void }) {
  const [attempt, setAttempt] = useState(0);
  const Body = lazyFor(kind, attempt);
  return (
    <Boundary key={attempt} onRemove={onRemove} onRetry={() => setAttempt((a) => a + 1)}>
      <Suspense fallback={<Skeleton />}>
        <Body kind={kind} size={size} active={active} />
      </Suspense>
    </Boundary>
  );
}
