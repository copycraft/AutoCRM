'use client';

import { useCallback, useMemo, useRef, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { yardApi, yardLayoutApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';
import { ErrorState } from '@/components/ui/ErrorState';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { cn } from '@/lib/utils/format';
import type { LayoutItem, YardVehicle, YardLocation } from '@/lib/api/types';

const CELL = 40;
const GRID_W = 40;
const GRID_H = 25;

type Tool = 'select' | 'spot' | 'building' | 'workshop_bay';

interface DragState {
  kind: 'move' | 'create';
  tool?: Tool;
  itemId?: number;
  startX: number;
  startY: number;
  curX: number;
  curY: number;
  origin?: { x: number; y: number };
}

function snap(v: number) {
  return Math.round(v / CELL) * CELL;
}

export function YardCanvas({ canMove, canConfigure }: { canMove: boolean; canConfigure: boolean }) {
  const t = useTranslations('yard3d');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();

  const [tool, setTool] = useState<Tool>('select');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  const layoutQ = useQuery({ queryKey: qk.yardLayout, queryFn: () => yardLayoutApi.list() });
  const boardQ = useQuery({
    queryKey: qk.yardBoard,
    queryFn: () => yardApi.board(),
    refetchInterval: 30_000,
    staleTime: 15_000,
  });

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: ['yard'] });
  };

  const saveItem = useMutation({
    mutationFn: (body: Parameters<typeof yardLayoutApi.save>[0]) => yardLayoutApi.save(body),
    onSuccess: invalidate,
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  const updateItem = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof yardLayoutApi.update>[1] }) =>
      yardLayoutApi.update(id, body),
    onSuccess: invalidate,
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  const deleteItem = useMutation({
    mutationFn: (id: number) => yardLayoutApi.remove(id),
    onSuccess: invalidate,
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  const moveVehicle = useMutation({
    mutationFn: (body: { vehicle_id: number; location_id: number | null; order_id: number | null }) =>
      yardApi.move(body),
    onSuccess: (r) => {
      if (r.over_capacity) toast('info', t('overCapacityToast'));
      invalidate();
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  const toGrid = useCallback((e: React.PointerEvent): { gx: number; gy: number } => {
    const svg = svgRef.current;
    if (!svg) return { gx: 0, gy: 0 };
    const rect = svg.getBoundingClientRect();
    const scaleX = (GRID_W * CELL) / rect.width;
    const scaleY = (GRID_H * CELL) / rect.height;
    return {
      gx: snap((e.clientX - rect.left) * scaleX),
      gy: snap((e.clientY - rect.top) * scaleY),
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    const { gx, gy } = toGrid(e);
    if (tool === 'select') return;
    setDrag({ kind: 'create', tool, startX: gx, startY: gy, curX: gx, curY: gy });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag) return;
    const { gx, gy } = toGrid(e);
    setDrag({ ...drag, curX: gx, curY: gy });
  };

  const onPointerUp = () => {
    if (!drag) return;
    if (drag.kind === 'create' && drag.tool && drag.tool !== 'select') {
      if (drag.tool === 'workshop_bay' && selectedId == null) {
        toast('info', t('selectBuildingFirst'));
        setDrag(null);
        return;
      }
      const x = Math.min(drag.startX, drag.curX);
      const y = Math.min(drag.startY, drag.curY);
      const w = Math.max(CELL, Math.abs(drag.curX - drag.startX));
      const h = Math.max(CELL, Math.abs(drag.curY - drag.startY));
      const kind = drag.tool;
      saveItem.mutate({
        kind,
        location_id: kind === 'spot' ? (boardQ.data?.locations[0]?.id ?? null) : undefined,
        parent_id: kind === 'workshop_bay' ? selectedId ?? undefined : undefined,
        label: '',
        number: '',
        x, y,
        width: w,
        height: h,
        rotation: 0,
        color: kind === 'spot' ? '#3b82f6' : kind === 'building' ? '#6b7280' : '#f59e0b',
      });
    }
    setDrag(null);
  };

  const onItemPointerDown = (e: React.PointerEvent, item: LayoutItem) => {
    if (!canConfigure || tool !== 'select') return;
    e.stopPropagation();
    setSelectedId(item.id);
    const { gx, gy } = toGrid(e);
    setDrag({ kind: 'move', itemId: item.id, startX: gx, startY: gy, curX: gx, curY: gy, origin: { x: item.x, y: item.y } });
  };

  const onItemPointerUp = () => {
    if (drag?.kind === 'move' && drag.itemId != null && drag.origin) {
      const nx = drag.origin.x + (drag.curX - drag.startX);
      const ny = drag.origin.y + (drag.curY - drag.startY);
      const item = layoutQ.data?.items.find((i) => i.id === drag.itemId);
      if (item && (item.x !== nx || item.y !== ny)) {
        updateItem.mutate({ id: drag.itemId, body: { x: nx, y: ny } });
      }
    }
    setDrag(null);
  };

  const selected = layoutQ.data?.items.find((i) => i.id === selectedId) ?? null;

  if (layoutQ.isPending || boardQ.isPending) return <DetailSkeleton />;
  if (layoutQ.isError) return <ErrorState error={layoutQ.error} onRetry={() => void layoutQ.refetch()} />;
  if (boardQ.isError) return <ErrorState error={boardQ.error} onRetry={() => void boardQ.refetch()} />;

  const items = layoutQ.data?.items ?? [];
  const vehicles = boardQ.data?.vehicles ?? [];
  const locations = boardQ.data?.locations ?? [];

  const vehiclesAt = (locationId: number | null) =>
    vehicles.filter((v) => (v.location_id ?? null) === locationId);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex gap-1 rounded-lg border border-steel-200 p-0.5">
          {(['select', 'spot', 'building', 'workshop_bay'] as Tool[]).map((tl) => {
            const disabled = tl === 'workshop_bay' && !items.some((i) => i.kind === 'building');
            return (
              <button
                key={tl}
                type="button"
                disabled={disabled}
                className={cn(
                  'rounded-md px-3 py-1 text-sm font-medium transition-colors',
                  tool === tl ? 'bg-steel-900 text-surface' : 'text-steel-600 hover:bg-steel-100',
                  disabled && 'cursor-not-allowed opacity-40 hover:bg-transparent',
                )}
                onClick={() => setTool(tl)}
                title={disabled ? t('selectBuildingFirst') : undefined}
              >
                {t(`tool_${tl}`)}
              </button>
            );
          })}
        </div>
        {canConfigure && selected && (
          <div className="flex items-center gap-2">
            <input
              className="input h-8 w-40 py-0"
              value={selected.number}
              placeholder={t('spotNumber')}
              onChange={(e) => updateItem.mutate({ id: selected.id, body: { number: e.target.value } })}
            />
            <input
              className="input h-8 w-40 py-0"
              value={selected.label}
              placeholder={t('spotLabel')}
              onChange={(e) => updateItem.mutate({ id: selected.id, body: { label: e.target.value } })}
            />
            <input
              type="color"
              className="h-8 w-10 cursor-pointer rounded border border-steel-200"
              value={selected.color}
              onChange={(e) => updateItem.mutate({ id: selected.id, body: { color: e.target.value } })}
            />
            <button
              type="button"
              className="btn-ghost btn-sm text-signal"
              onClick={() => deleteItem.mutate(selected.id)}
            >
              {t('delete')}
            </button>
          </div>
        )}
        {tool !== 'select' && <span className="text-metadata text-steel-500">{t('drawHint')}</span>}
      </div>

      <div
        className="overflow-hidden rounded-xl border border-steel-200 bg-panel"
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes('application/x-autocrm-vehicle')) {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
          }
        }}
        onDrop={(e) => {
          const raw = e.dataTransfer.getData('application/x-autocrm-vehicle');
          if (!raw) return;
          e.preventDefault();
          const { vehicle_id, order_id } = JSON.parse(raw);
          const rect = svgRef.current?.getBoundingClientRect();
          if (!rect) return;
          const scaleX = (GRID_W * CELL) / rect.width;
          const scaleY = (GRID_H * CELL) / rect.height;
          const gx = snap((e.clientX - rect.left) * scaleX);
          const gy = snap((e.clientY - rect.top) * scaleY);
          const spot = items.find((item) => {
            if (item.kind !== 'spot' && item.kind !== 'workshop_bay') return false;
            const w = item.rotation % 180 === 0 ? item.width : item.height;
            const h = item.rotation % 180 === 0 ? item.height : item.width;
            return gx >= item.x && gx <= item.x + w && gy >= item.y && gy <= item.y + h;
          });
          if (spot) {
            moveVehicle.mutate({ vehicle_id, location_id: spot.location_id ?? null, order_id });
          }
        }}
      >
        <svg
          ref={svgRef}
          viewBox={`0 0 ${GRID_W * CELL} ${GRID_H * CELL}`}
          className={cn('block w-full', tool !== 'select' ? 'cursor-crosshair' : 'cursor-default')}
          style={{ aspectRatio: `${GRID_W * CELL} / ${GRID_H * CELL}` }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
        >
          <defs>
            <pattern id="grid" width={CELL} height={CELL} patternUnits="userSpaceOnUse">
              <path d={`M ${CELL} 0 L 0 0 0 ${CELL}`} fill="none" stroke="#e5e7eb" strokeWidth="0.5" />
            </pattern>
          </defs>
          <rect width={GRID_W * CELL} height={GRID_H * CELL} fill="url(#grid)" />

          {items.map((item) => {
            const isSelected = item.id === selectedId;
            const w = item.rotation % 180 === 0 ? item.width : item.height;
            const h = item.rotation % 180 === 0 ? item.height : item.width;
            const here = item.kind === 'spot' || item.kind === 'workshop_bay'
              ? vehiclesAt(item.location_id ?? null)
              : [];
            return (
              <g
                key={item.id}
                transform={`translate(${item.x},${item.y}) rotate(${item.rotation},${w / 2},${h / 2})`}
                onPointerDown={(e) => onItemPointerDown(e, item)}
                className={canConfigure && tool === 'select' ? 'cursor-move' : undefined}
              >
                <rect
                  width={w}
                  height={h}
                  rx={4}
                  fill={item.kind === 'building' ? '#d1d5db' : item.color + '22'}
                  stroke={isSelected ? '#1c1917' : item.color}
                  strokeWidth={isSelected ? 2.5 : 1.5}
                  strokeDasharray={item.kind === 'building' ? '6 3' : undefined}
                />
                <text
                  x={w / 2}
                  y={h / 2 - (item.label ? 4 : -4)}
                  textAnchor="middle"
                  dominantBaseline="central"
                  className="fill-steel-900 text-[11px] font-semibold select-none"
                >
                  {item.number || item.label || (item.kind === 'building' ? t('building') : '')}
                </text>
                {item.label && (
                  <text
                    x={w / 2}
                    y={h / 2 + 10}
                    textAnchor="middle"
                    dominantBaseline="central"
                    className="fill-steel-500 text-[9px] select-none"
                  >
                    {item.label}
                  </text>
                )}
                {here.length > 0 && (
                  <g transform={`translate(4, ${h - 18})`}>
                    {here.slice(0, 3).map((v, i) => (
                      <g key={v.vehicle_id} transform={`translate(${i * 52}, 0)`}>
                        <rect width={48} height={14} rx={3} fill="#1c1917" />
                        <text x={24} y={10} textAnchor="middle" dominantBaseline="central" className="fill-white text-[8px] font-mono select-none">
                          {v.plate ?? `#${v.vehicle_id}`}
                        </text>
                      </g>
                    ))}
                  </g>
                )}
              </g>
            );
          })}

          {drag?.kind === 'create' && drag.tool && drag.tool !== 'select' && (
            <rect
              x={Math.min(drag.startX, drag.curX)}
              y={Math.min(drag.startY, drag.curY)}
              width={Math.max(CELL, Math.abs(drag.curX - drag.startX))}
              height={Math.max(CELL, Math.abs(drag.curY - drag.startY))}
              fill="#3b82f622"
              stroke="#3b82f6"
              strokeWidth={1.5}
              strokeDasharray="4 2"
              rx={4}
            />
          )}
        </svg>
      </div>

      {canMove && vehicles.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {vehicles.filter((v) => v.location_id == null).map((v) => (
            <div
              key={v.vehicle_id}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData('application/x-autocrm-vehicle', JSON.stringify({
                  vehicle_id: v.vehicle_id,
                  order_id: v.order_id,
                }));
                e.dataTransfer.effectAllowed = 'move';
              }}
              className="cursor-grab rounded-lg border border-steel-200 bg-surface px-3 py-1.5 text-sm shadow-panel active:cursor-grabbing"
            >
              <span className="font-mono font-semibold">{v.plate ?? `#${v.vehicle_id}`}</span>
              <span className="ml-2 text-metadata text-steel-500">{t('dragToPlace')}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
