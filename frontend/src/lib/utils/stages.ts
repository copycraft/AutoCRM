import type { StageDefinition } from '@/lib/api/types';
import type { StatusTone } from '@/components/ui/StatusBadge';

/**
 * Badge tone from backend stage flags — never hardcoded stage names.
 * Neutral steel by default; terminal (completed) reads done; exit stages
 * (cancelled/lost) are muted, not alerts — a cancelled order isn't one.
 */
export function stageTone(def: StageDefinition | undefined): StatusTone {
  if (!def) return 'steel';
  if (def.is_exit) return 'muted';
  if (def.is_terminal) return 'done';
  return 'steel';
}
