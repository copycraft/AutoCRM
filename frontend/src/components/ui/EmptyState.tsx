import { Inbox } from 'lucide-react';

export function EmptyState({ title, hint, action }: { title: string; hint?: string; action?: React.ReactNode }) {
  // Real empty states, never fake data (FRONTEND_PLAN.md §5 M5).
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-steel-200 bg-surface px-6 py-12 text-center">
      <Inbox className="h-8 w-8 text-steel-500" aria-hidden />
      <p className="text-sm font-medium">{title}</p>
      {hint && <p className="text-sm text-steel-500 max-w-md">{hint}</p>}
      {action && <div className="mt-3">{action}</div>}
    </div>
  );
}
