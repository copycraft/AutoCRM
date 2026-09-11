import { TriangleAlert } from 'lucide-react';
import { errorMessage } from '@/lib/api/errors';

export function ErrorState({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => void;
}) {
  const message = errorMessage(error, 'Ismeretlen hiba történt.');
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-xl border border-signal/30 bg-signal/5 px-6 py-12 text-center" role="alert">
      <TriangleAlert className="h-8 w-8 text-signal" aria-hidden />
      <p className="text-sm font-medium">Hiba történt</p>
      <p className="text-sm text-steel-900 max-w-md">{message}</p>
      {onRetry && (
        <button className="btn-secondary btn-sm mt-3" onClick={onRetry}>
          Újrapróbálkozás
        </button>
      )}
    </div>
  );
}
