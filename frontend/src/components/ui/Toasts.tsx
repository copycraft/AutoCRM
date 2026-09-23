'use client';

import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react';
import { CheckCircle2, AlertTriangle, Info, X } from 'lucide-react';

export type ToastTone = 'success' | 'error' | 'info';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  tone: ToastTone;
  title: string;
  body?: string;
  action?: ToastAction;
}

interface ToastApi {
  toast: (tone: ToastTone, title: string, body?: string, action?: ToastAction) => void;
  success: (title: string, body?: string, action?: ToastAction) => void;
  error: (title: string, body?: string) => void;
  info: (title: string, body?: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) {
    // Outside the provider (tests, isolated renders): no-op so callers stay simple.
    return {
      toast: () => undefined,
      success: () => undefined,
      error: () => undefined,
      info: () => undefined,
    };
  }
  return api;
}

function ToneIcon({ tone }: { tone: ToastTone }) {
  if (tone === 'success') return <CheckCircle2 className="h-4 w-4 shrink-0" aria-hidden />;
  if (tone === 'error') return <AlertTriangle className="h-4 w-4 shrink-0" aria-hidden />;
  return <Info className="h-4 w-4 shrink-0" aria-hidden />;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const idRef = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    (tone: ToastTone, title: string, body?: string, action?: ToastAction) => {
      const id = idRef.current++;
      setToasts((prev) => [...prev.slice(-3), { id, tone, title, body, action }]);
      // Actionable toasts (undo) linger longer.
      window.setTimeout(() => dismiss(id), action ? 8000 : tone === 'error' ? 7000 : 4000);
    },
    [dismiss],
  );

  const api = useMemo<ToastApi>(
    () => ({
      toast,
      success: (title, body, action) => toast('success', title, body, action),
      error: (title, body) => toast('error', title, body),
      info: (title, body) => toast('info', title, body),
    }),
    [toast],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div
        aria-live="polite"
        className="pointer-events-none fixed bottom-4 right-4 z-[100] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            role={t.tone === 'error' ? 'alert' : 'status'}
            className="card pointer-events-auto flex items-start gap-2 p-3 shadow-lg"
          >
            <span
              className={
                t.tone === 'success'
                  ? 'text-done'
                  : t.tone === 'error'
                    ? 'text-signal'
                    : 'text-steel-500'
              }
            >
              <ToneIcon tone={t.tone} />
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-body font-medium">{t.title}</p>
              {t.body && <p className="mt-0.5 break-words text-metadata text-steel-500">{t.body}</p>}
              {t.action && (
                <button
                  type="button"
                  className="mt-1 text-body font-medium underline"
                  onClick={() => {
                    t.action?.onClick();
                    dismiss(t.id);
                  }}
                >
                  {t.action.label}
                </button>
              )}
            </div>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              aria-label="Bezárás"
              className="rounded p-0.5 text-steel-500 hover:text-steel-900"
            >
              <X className="h-4 w-4" aria-hidden />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
