'use client';

// Drawing over a photo: arrows, circles, boxes, freehand and labels, kept as shapes in
// fractions of the picture (0..1) and drawn as an overlay. The photo itself is never
// changed, which is what lets the same tool work on write-once intake evidence.

import { useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { useTranslations } from 'next-intl';
import { ArrowUpRight, Circle, Eraser, Minus, Pencil, Square, Type, Undo2 } from 'lucide-react';
import { cn } from '@/lib/utils/format';

export type Shape =
  | { type: 'arrow' | 'line'; points: [number, number, number, number]; color: string }
  | { type: 'rect' | 'ellipse'; x: number; y: number; w: number; h: number; color: string }
  | { type: 'pen'; points: [number, number][]; color: string }
  | { type: 'text'; x: number; y: number; text: string; color: string };

export const COLORS = ['#e11d48', '#facc15', '#ffffff', '#1b2327', '#0f5c7a'];

type Tool = Shape['type'];

/** Shapes from the server, keeping only what this editor can draw. */
export function parseShapes(raw: unknown): Shape[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter((s): s is Shape => {
    if (!s || typeof s !== 'object') return false;
    const t = (s as { type?: unknown }).type;
    return ['arrow', 'line', 'rect', 'ellipse', 'pen', 'text'].includes(t as string);
  });
}

function ArrowHead({ x1, y1, x2, y2, color, size }: { x1: number; y1: number; x2: number; y2: number; color: string; size: number }) {
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const a1 = angle + Math.PI - 0.45;
  const a2 = angle + Math.PI + 0.45;
  const p = `${x2},${y2} ${x2 + size * Math.cos(a1)},${y2 + size * Math.sin(a1)} ${x2 + size * Math.cos(a2)},${y2 + size * Math.sin(a2)}`;
  return <polygon points={p} fill={color} />;
}

/** The shapes in pixels over a picture of `width` × `height`. */
export function ShapesSvg({ shapes, width, height }: { shapes: Shape[]; width: number; height: number }) {
  if (!width || !height) return null;
  const stroke = Math.max(2, Math.round(Math.min(width, height) / 220));
  return (
    <svg
      className="pointer-events-none absolute inset-0"
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      aria-hidden
    >
      {shapes.map((s, i) => {
        const outline = { stroke: s.color, strokeWidth: stroke, fill: 'none', strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const };
        switch (s.type) {
          case 'arrow':
          case 'line': {
            const [x1, y1, x2, y2] = [s.points[0] * width, s.points[1] * height, s.points[2] * width, s.points[3] * height];
            return (
              <g key={i}>
                <line x1={x1} y1={y1} x2={x2} y2={y2} {...outline} />
                {s.type === 'arrow' && <ArrowHead x1={x1} y1={y1} x2={x2} y2={y2} color={s.color} size={stroke * 6} />}
              </g>
            );
          }
          case 'rect':
            return <rect key={i} x={s.x * width} y={s.y * height} width={s.w * width} height={s.h * height} {...outline} />;
          case 'ellipse':
            return (
              <ellipse
                key={i}
                cx={(s.x + s.w / 2) * width}
                cy={(s.y + s.h / 2) * height}
                rx={(Math.abs(s.w) / 2) * width}
                ry={(Math.abs(s.h) / 2) * height}
                {...outline}
              />
            );
          case 'pen':
            return <polyline key={i} points={s.points.map(([x, y]) => `${x * width},${y * height}`).join(' ')} {...outline} />;
          case 'text':
            return (
              <text
                key={i}
                x={s.x * width}
                y={s.y * height}
                fill={s.color}
                stroke={s.color === '#ffffff' ? '#1b2327' : '#ffffff'}
                strokeWidth={stroke / 2}
                paintOrder="stroke"
                fontSize={Math.max(14, stroke * 8)}
                fontWeight={600}
              >
                {s.text}
              </text>
            );
          default:
            return null;
        }
      })}
    </svg>
  );
}

const clamp = (v: number) => Math.min(1, Math.max(0, v));

/** Draws on top of a picture of `width` × `height`; Save hands back every shape. */
export function AnnotationEditor({
  initial,
  width,
  height,
  saving,
  onSave,
  onCancel,
}: {
  initial: Shape[];
  width: number;
  height: number;
  saving: boolean;
  onSave: (shapes: Shape[]) => void;
  onCancel: () => void;
}) {
  const t = useTranslations('media');
  const tc = useTranslations('common');
  const [shapes, setShapes] = useState<Shape[]>(initial);
  const [tool, setTool] = useState<Tool>('arrow');
  const [color, setColor] = useState<string>(COLORS[0] ?? '#e11d48');
  const [draft, setDraft] = useState<Shape | null>(null);
  const start = useRef<[number, number] | null>(null);
  const surface = useRef<HTMLDivElement>(null);

  const at = (e: ReactPointerEvent): [number, number] => {
    const box = surface.current!.getBoundingClientRect();
    return [clamp((e.clientX - box.left) / box.width), clamp((e.clientY - box.top) / box.height)];
  };

  const down = (e: ReactPointerEvent) => {
    const [x, y] = at(e);
    if (tool === 'text') {
      const text = window.prompt(t('labelPrompt'))?.trim();
      if (text) setShapes((all) => [...all, { type: 'text', x, y, text: text.slice(0, 200), color }]);
      return;
    }
    (e.target as Element).setPointerCapture?.(e.pointerId);
    start.current = [x, y];
    setDraft(
      tool === 'pen'
        ? { type: 'pen', points: [[x, y]], color }
        : tool === 'arrow' || tool === 'line'
          ? { type: tool, points: [x, y, x, y], color }
          : { type: tool, x, y, w: 0, h: 0, color },
    );
  };

  const move = (e: ReactPointerEvent) => {
    if (!draft || !start.current) return;
    const [x, y] = at(e);
    const [sx, sy] = start.current;
    if (draft.type === 'pen') setDraft({ ...draft, points: [...draft.points, [x, y]] });
    else if (draft.type === 'arrow' || draft.type === 'line') setDraft({ ...draft, points: [sx, sy, x, y] });
    else if (draft.type === 'rect' || draft.type === 'ellipse')
      setDraft({ ...draft, x: Math.min(sx, x), y: Math.min(sy, y), w: Math.abs(x - sx), h: Math.abs(y - sy) });
  };

  const up = () => {
    if (draft) {
      const tiny =
        (draft.type === 'rect' || draft.type === 'ellipse') && draft.w < 0.005 && draft.h < 0.005;
      if (!tiny) setShapes((all) => [...all, draft]);
    }
    setDraft(null);
    start.current = null;
  };

  const tools: { key: Tool; icon: typeof ArrowUpRight }[] = [
    { key: 'arrow', icon: ArrowUpRight },
    { key: 'line', icon: Minus },
    { key: 'ellipse', icon: Circle },
    { key: 'rect', icon: Square },
    { key: 'pen', icon: Pencil },
    { key: 'text', icon: Type },
  ];

  return (
    <>
      <div
        ref={surface}
        className="absolute inset-0 cursor-crosshair touch-none"
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
      >
        <ShapesSvg shapes={draft ? [...shapes, draft] : shapes} width={width} height={height} />
      </div>
      <div className="absolute left-1/2 top-2 z-10 flex -translate-x-1/2 flex-wrap items-center gap-1 rounded-lg bg-surface/95 p-1 shadow-card">
        {tools.map(({ key, icon: Icon }) => (
          <button
            key={key}
            type="button"
            className={cn('btn-ghost btn-sm', tool === key && 'bg-steel-900 text-surface hover:bg-steel-900')}
            onClick={() => setTool(key)}
            title={t(`tools.${key}`)}
            aria-label={t(`tools.${key}`)}
            aria-pressed={tool === key}
          >
            <Icon className="h-4 w-4" aria-hidden />
          </button>
        ))}
        <span className="mx-1 h-6 w-px bg-steel-200" aria-hidden />
        {COLORS.map((c) => (
          <button
            key={c}
            type="button"
            className={cn('h-6 w-6 rounded-full border-2', color === c ? 'border-steel-900' : 'border-steel-200')}
            style={{ backgroundColor: c }}
            onClick={() => setColor(c)}
            aria-label={c}
            aria-pressed={color === c}
          />
        ))}
        <span className="mx-1 h-6 w-px bg-steel-200" aria-hidden />
        <button type="button" className="btn-ghost btn-sm" onClick={() => setShapes((all) => all.slice(0, -1))} disabled={!shapes.length} title={t('undo')} aria-label={t('undo')}>
          <Undo2 className="h-4 w-4" aria-hidden />
        </button>
        <button type="button" className="btn-ghost btn-sm" onClick={() => setShapes([])} disabled={!shapes.length} title={t('clearDrawing')} aria-label={t('clearDrawing')}>
          <Eraser className="h-4 w-4" aria-hidden />
        </button>
        <button type="button" className="btn-ghost btn-sm" onClick={onCancel} disabled={saving}>
          {tc('cancel')}
        </button>
        <button type="button" className="btn-primary btn-sm" onClick={() => onSave(shapes)} disabled={saving}>
          {saving ? tc('saving') : tc('save')}
        </button>
      </div>
    </>
  );
}
