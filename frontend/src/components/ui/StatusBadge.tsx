import { cn } from '@/lib/utils/format';

type Tone = 'signal' | 'cold' | 'done' | 'steel';

export function StatusBadge({ tone, children }: { tone: Tone; children: React.ReactNode }) {
  return (
    <span
      className={cn(
        'badge',
        tone === 'signal' && 'bg-signal/10 text-signal',
        tone === 'cold' && 'bg-cold/10 text-cold',
        tone === 'done' && 'bg-done/10 text-done',
        tone === 'steel' && 'bg-steel-200 text-steel-900',
      )}
    >
      {children}
    </span>
  );
}
