import type { StageDefinition } from '@/lib/api/types';

/** Badge tone from backend stage flags — never hardcoded stage names. */
export function stageTone(def: StageDefinition | undefined): 'signal' | 'cold' | 'done' | 'steel' {
  if (!def) return 'steel';
  if (def.is_exit) return 'signal';
  if (def.is_terminal) return 'done';
  return 'cold';
}
