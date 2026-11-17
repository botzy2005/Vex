import { useCallback, useEffect, useRef, useState } from 'react';
import type { Backend } from '../backend/backend';
import type { AiEvent, ChatMessage, ContextPayload, ConversationSummary } from '../shared/types';
import type { AssistantHook } from '../backend/hooks';

export interface ChatError {
  message: string;
  /** Provider error kind from the backend ('auth', 'network', 'config'…) when known. */
  kind: string | null;
}

export interface ChatHook {
  conversationId: string | null;
  title: string;
  messages: ChatMessage[];
  /** Text of the reply that is arriving right now (null when nothing is streaming). */
  streaming: string | null;
  /** 'waiting' = request sent, no text yet; 'streaming' = text arriving. */
  phase: 'idle' | 'waiting' | 'streaming';
  error: ChatError | null;
  history: ConversationSummary[];
  send(text: string, context: ContextPayload | null): Promise<boolean>;
  stop(): void;
  newChat(): void;
  open(id: string): Promise<void>;
  remove(id: string): Promise<void>;
  clearAll(): Promise<void>;
  dismissError(): void;
}

/**
 * Chat state for the island. Conversations live in the native app (so they survive restarts);
 * this hook mirrors the open one and applies streaming events. After every finished or stopped
 * reply it re-reads the stored conversation, so what you see is always what was saved.
 */
export function useChat(backend: Backend, assistant: AssistantHook): ChatHook {
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [title, setTitle] = useState('New chat');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState<string | null>(null);
  const [phase, setPhase] = useState<ChatHook['phase']>('idle');
  const [error, setError] = useState<ChatError | null>(null);
  const [history, setHistory] = useState<ConversationSummary[]>([]);

  const requestId = useRef<number | null>(null);
  const convRef = useRef<string | null>(null);
  convRef.current = conversationId;
  const setFace = useRef(assistant.set);
  setFace.current = assistant.set;

  const refreshHistory = useCallback(async () => {
    try {
      setHistory(await backend.chatList());
    } catch {
      /* history list is a convenience */
    }
  }, [backend]);

  const load = useCallback(
    async (id: string) => {
      const c = await backend.chatGet(id);
      if (!c) return false;
      setConversationId(c.id);
      setTitle(c.title);
      setMessages(c.messages);
      return true;
    },
    [backend],
  );

  // Reopen the most recent conversation on start, so chats survive a restart.
  useEffect(() => {
    let dead = false;
    (async () => {
      try {
        const list = await backend.chatList();
        if (dead) return;
        setHistory(list);
        if (list[0]) await load(list[0].id);
      } catch {
        /* start empty */
      }
    })();
    return () => {
      dead = true;
    };
  }, [backend, load]);

  useEffect(() => {
    let off: (() => void) | undefined;
    let dead = false;
    backend
      .onAiEvent((e: AiEvent) => {
        if (e.requestId !== requestId.current) return;
        if (e.kind === 'delta') {
          setPhase('streaming');
          setStreaming((s) => (s ?? '') + (e.text ?? ''));
          return;
        }
        // finished in some way: the stored conversation is the source of truth
        requestId.current = null;
        setStreaming(null);
        setPhase('idle');
        void load(e.conversationId).then(refreshHistory);
        if (e.kind === 'error') {
          setError({ message: e.message ?? 'The request failed.', kind: e.errorKind ?? null });
          setFace.current('error', e.message ?? 'The request failed.');
        } else {
          setFace.current('idle');
        }
      })
      .then((u) => (dead ? u() : (off = u)));
    return () => {
      dead = true;
      off?.();
    };
  }, [backend, load, refreshHistory]);

  const send = useCallback(
    async (text: string, context: ContextPayload | null) => {
      setError(null);
      try {
        const r = await backend.aiSend({ conversationId: convRef.current, text, context });
        requestId.current = r.requestId;
        setConversationId(r.conversationId);
        setPhase('waiting');
        setStreaming(null);
        setFace.current('thinking');
        await load(r.conversationId); // shows the stored user message immediately
        void refreshHistory();
        return true;
      } catch (e) {
        // Nothing was started and nothing was stored (not configured, still answering…).
        const message = e instanceof Error ? e.message : String(e);
        setError({ message, kind: 'config' });
        return false;
      }
    },
    [backend, load, refreshHistory],
  );

  const stop = useCallback(() => void backend.aiCancel(), [backend]);

  const newChat = useCallback(() => {
    if (requestId.current !== null) return; // finish or stop the current answer first
    setConversationId(null);
    setTitle('New chat');
    setMessages([]);
    setError(null);
  }, []);

  const open = useCallback(
    async (id: string) => {
      if (requestId.current !== null) return;
      setError(null);
      await load(id);
    },
    [load],
  );

  const remove = useCallback(
    async (id: string) => {
      await backend.chatDelete(id);
      if (convRef.current === id) {
        setConversationId(null);
        setTitle('New chat');
        setMessages([]);
      }
      await refreshHistory();
    },
    [backend, refreshHistory],
  );

  const clearAll = useCallback(async () => {
    await backend.chatClearAll();
    setConversationId(null);
    setTitle('New chat');
    setMessages([]);
    setError(null);
    await refreshHistory();
  }, [backend, refreshHistory]);

  return { conversationId, title, messages, streaming, phase, error, history, send, stop, newChat, open, remove, clearAll, dismissError: () => setError(null) };
}
