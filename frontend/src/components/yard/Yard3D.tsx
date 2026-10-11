'use client';

import { useEffect, useRef, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { yardApi, yardLayoutApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { ErrorState } from '@/components/ui/ErrorState';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import type { LayoutItem, YardVehicle } from '@/lib/api/types';

const SCALE = 0.1;

function itemDims(item: LayoutItem): { w: number; h: number } {
  return item.rotation % 180 === 0
    ? { w: item.width, h: item.height }
    : { w: item.height, h: item.width };
}

function makeCar(color: string): THREE.Group {
  const g = new THREE.Group();

  const bodyMat = new THREE.MeshStandardMaterial({ color, metalness: 0.8, roughness: 0.25 });
  const glassMat = new THREE.MeshStandardMaterial({ color: '#0f172a', metalness: 0.95, roughness: 0.05 });
  const lightMat = new THREE.MeshStandardMaterial({ color: '#fbbf24', emissive: '#fbbf24', emissiveIntensity: 0.4 });
  const tailMat = new THREE.MeshStandardMaterial({ color: '#ef4444', emissive: '#ef4444', emissiveIntensity: 0.3 });
  const wheelMat = new THREE.MeshStandardMaterial({ color: '#0c0a09', roughness: 0.85 });
  const hubMat = new THREE.MeshStandardMaterial({ color: '#a8a29e', metalness: 0.9, roughness: 0.3 });

  const lowerBody = new THREE.Mesh(new THREE.BoxGeometry(1.85, 0.45, 4.3), bodyMat);
  lowerBody.position.y = 0.38;
  lowerBody.castShadow = true;
  lowerBody.receiveShadow = true;
  g.add(lowerBody);

  const hood = new THREE.Mesh(new THREE.BoxGeometry(1.75, 0.15, 1.0), bodyMat);
  hood.position.set(0, 0.65, 1.55);
  hood.castShadow = true;
  g.add(hood);

  const cabin = new THREE.Mesh(new THREE.BoxGeometry(1.65, 0.5, 2.1), glassMat);
  cabin.position.set(0, 0.85, -0.25);
  cabin.castShadow = true;
  g.add(cabin);

  const roof = new THREE.Mesh(new THREE.BoxGeometry(1.55, 0.06, 1.8), bodyMat);
  roof.position.set(0, 1.12, -0.25);
  roof.castShadow = true;
  g.add(roof);

  const frontL = new THREE.Mesh(new THREE.BoxGeometry(0.25, 0.1, 0.04), lightMat);
  frontL.position.set(-0.6, 0.5, 2.16);
  g.add(frontL);
  const frontR = frontL.clone();
  frontR.position.x = 0.6;
  g.add(frontR);

  const tailL = new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.1, 0.04), tailMat);
  tailL.position.set(-0.65, 0.55, -2.16);
  g.add(tailL);
  const tailR = tailL.clone();
  tailR.position.x = 0.65;
  g.add(tailR);

  const wheelGeo = new THREE.CylinderGeometry(0.33, 0.33, 0.24, 24);
  const hubGeo = new THREE.CylinderGeometry(0.16, 0.16, 0.26, 12);
  const offsets: [number, number][] = [
    [-0.88, 1.35], [0.88, 1.35], [-0.88, -1.35], [0.88, -1.35],
  ];
  for (const [x, z] of offsets) {
    const wheel = new THREE.Mesh(wheelGeo, wheelMat);
    wheel.rotation.z = Math.PI / 2;
    wheel.position.set(x, 0.33, z);
    wheel.castShadow = true;
    g.add(wheel);
    const hub = new THREE.Mesh(hubGeo, hubMat);
    hub.rotation.z = Math.PI / 2;
    hub.position.set(x, 0.33, z);
    g.add(hub);
  }

  return g;
}

export function Yard3D() {
  const containerRef = useRef<HTMLDivElement>(null);

  const layoutQ = useQuery({ queryKey: qk.yardLayout, queryFn: () => yardLayoutApi.list() });
  const boardQ = useQuery({
    queryKey: qk.yardBoard,
    queryFn: () => yardApi.board(),
    refetchInterval: 30_000,
    staleTime: 15_000,
  });

  const items = layoutQ.data?.items ?? [];
  const vehicles = boardQ.data?.vehicles ?? [];
  const maxX = useMemo(() => Math.max(200, ...items.map((i) => i.x + i.width)) * SCALE, [items]);
  const maxZ = useMemo(() => Math.max(200, ...items.map((i) => i.y + i.height)) * SCALE, [items]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || layoutQ.isPending || boardQ.isPending || layoutQ.isError || boardQ.isError) return;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color('#c7d2fe');
    scene.fog = new THREE.Fog('#c7d2fe', 80, 250);

    const camera = new THREE.PerspectiveCamera(45, container.clientWidth / container.clientHeight, 0.1, 500);
    const camDist = Math.max(maxX, maxZ) * 0.9 + 30;
    camera.position.set(maxX * 0.5 + camDist * 0.6, camDist * 0.7, maxZ * 0.5 + camDist * 0.8);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(container.clientWidth, container.clientHeight);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.1;
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(maxX / 2, 0, maxZ / 2);
    controls.enableDamping = true;
    controls.dampingFactor = 0.06;
    controls.maxPolarAngle = Math.PI / 2.05;
    controls.minDistance = 10;
    controls.maxDistance = 200;
    controls.update();

    scene.add(new THREE.HemisphereLight('#e0e7ff', '#78716c', 0.7));

    const sun = new THREE.DirectionalLight('#fffbeb', 1.8);
    sun.position.set(maxX + 30, 50, maxZ + 30);
    sun.castShadow = true;
    sun.shadow.mapSize.width = 4096;
    sun.shadow.mapSize.height = 4096;
    sun.shadow.camera.near = 0.5;
    sun.shadow.camera.far = 200;
    sun.shadow.camera.left = -maxX - 30;
    sun.shadow.camera.right = maxX + 30;
    sun.shadow.camera.top = maxZ + 30;
    sun.shadow.camera.bottom = -maxZ - 30;
    sun.shadow.bias = -0.0004;
    scene.add(sun);

    const fill = new THREE.DirectionalLight('#dbeafe', 0.4);
    fill.position.set(-20, 30, -20);
    scene.add(fill);

    const groundSize = Math.max(maxX, maxZ) + 80;
    const ground = new THREE.Mesh(
      new THREE.PlaneGeometry(groundSize, groundSize),
      new THREE.MeshStandardMaterial({ color: '#d6d3d1', roughness: 1, metalness: 0 }),
    );
    ground.rotation.x = -Math.PI / 2;
    ground.position.set(maxX / 2, -0.02, maxZ / 2);
    ground.receiveShadow = true;
    scene.add(ground);

    const asphalt = new THREE.Mesh(
      new THREE.PlaneGeometry(maxX + 10, maxZ + 10),
      new THREE.MeshStandardMaterial({ color: '#a8a29e', roughness: 0.95 }),
    );
    asphalt.rotation.x = -Math.PI / 2;
    asphalt.position.set(maxX / 2, 0, maxZ / 2);
    asphalt.receiveShadow = true;
    scene.add(asphalt);

    const grid = new THREE.GridHelper(groundSize, Math.round(groundSize / 4), '#78716c', '#c7c0ba');
    grid.position.set(maxX / 2, 0.01, maxZ / 2);
    (grid.material as THREE.Material).opacity = 0.3;
    (grid.material as THREE.Material).transparent = true;
    scene.add(grid);

    const carColors = ['#3b82f6', '#ef4444', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];

    for (const item of items) {
      const { w, h } = itemDims(item);
      const cx = (item.x + item.width / 2) * SCALE;
      const cz = (item.y + item.height / 2) * SCALE;
      const angle = (item.rotation * Math.PI) / 180;

      if (item.kind === 'building') {
        const wallMat = new THREE.MeshStandardMaterial({ color: '#d6d3d1', roughness: 0.85 });
        const accentMat = new THREE.MeshStandardMaterial({ color: '#78716c', roughness: 0.7 });
        const wallH = 5;

        const walls = new THREE.Mesh(new THREE.BoxGeometry(w * SCALE, wallH, h * SCALE), wallMat);
        walls.position.set(cx, wallH / 2, cz);
        walls.castShadow = true;
        walls.receiveShadow = true;
        scene.add(walls);

        const roof = new THREE.Mesh(new THREE.BoxGeometry(w * SCALE + 0.6, 0.4, h * SCALE + 0.6), accentMat);
        roof.position.set(cx, wallH + 0.2, cz);
        roof.castShadow = true;
        scene.add(roof);

        const doorW = Math.min(1.5, w * SCALE * 0.2);
        const doorH = 2.5;
        const door = new THREE.Mesh(
          new THREE.BoxGeometry(doorW, doorH, 0.1),
          new THREE.MeshStandardMaterial({ color: '#44403c', roughness: 0.6 }),
        );
        door.position.set(cx, doorH / 2, cz + h * SCALE / 2 + 0.06);
        scene.add(door);

        const winMat = new THREE.MeshStandardMaterial({
          color: '#bfdbfe', metalness: 0.9, roughness: 0.1,
          transparent: true, opacity: 0.6,
        });
        const winCount = Math.max(1, Math.floor((w * SCALE) / 2.5));
        for (let i = 0; i < winCount; i++) {
          const wx = cx - (w * SCALE) / 2 + (i + 0.5) * (w * SCALE / winCount);
          const win = new THREE.Mesh(new THREE.BoxGeometry(0.8, 1.2, 0.08), winMat);
          win.position.set(wx, 2.8, cz + h * SCALE / 2 + 0.05);
          scene.add(win);
        }
      } else {
        const isBay = item.kind === 'workshop_bay';
        const mat = new THREE.MeshStandardMaterial({
          color: new THREE.Color(item.color),
          roughness: 0.6,
          transparent: true,
          opacity: isBay ? 0.3 : 0.15,
        });
        const pad = new THREE.Mesh(new THREE.BoxGeometry(w * SCALE, 0.12, h * SCALE), mat);
        pad.position.set(cx, 0.06, cz);
        pad.receiveShadow = true;
        scene.add(pad);

        const lineMat = new THREE.MeshBasicMaterial({ color: item.color });
        const t = 0.15;
        const lw = w * SCALE;
        const lh = h * SCALE;
        const edges: [number, number, number, number][] = [
          [0, lh / 2, lw, t], [0, -lh / 2, lw, t],
          [lw / 2, 0, t, lh], [-lw / 2, 0, t, lh],
        ];
        for (const [lx, lz, ew, eh] of edges) {
          const line = new THREE.Mesh(new THREE.BoxGeometry(ew, 0.03, eh), lineMat);
          line.position.set(cx + lx, 0.14, cz + lz);
          scene.add(line);
        }

        const here = vehicles.filter((v) => v.location_id === item.location_id);
        here.forEach((v, i) => {
          const car = makeCar(carColors[i % carColors.length] ?? '#3b82f6');
          car.rotation.y = angle;
          const offset = here.length > 1 ? (i - (here.length - 1) / 2) * 1.4 : 0;
          const ox = Math.sin(angle) * offset;
          const oz = Math.cos(angle) * offset;
          car.position.set(cx + ox, 0.12, cz + oz);
          scene.add(car);
        });
      }
    }

    const unplaced = vehicles.filter((v) => v.location_id == null);
    unplaced.forEach((v, i) => {
      const car = makeCar('#78716c');
      const row = Math.floor(i / 6);
      car.position.set(-5 - (i % 6) * 2.5, 0.12, -5 - row * 3);
      scene.add(car);
    });

    let raf = 0;
    const animate = () => {
      raf = requestAnimationFrame(animate);
      controls.update();
      renderer.render(scene, camera);
    };
    animate();

    const onResize = () => {
      camera.aspect = container.clientWidth / container.clientHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(container.clientWidth, container.clientHeight);
    };
    const resizeObs = new ResizeObserver(onResize);
    resizeObs.observe(container);

    return () => {
      cancelAnimationFrame(raf);
      resizeObs.disconnect();
      controls.dispose();
      renderer.dispose();
      scene.traverse((obj) => {
        if (obj instanceof THREE.Mesh) {
          obj.geometry.dispose();
          const m = obj.material;
          if (Array.isArray(m)) m.forEach((mm) => mm.dispose());
          else m.dispose();
        }
      });
      if (renderer.domElement.parentNode === container) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [items, vehicles, maxX, maxZ, layoutQ.isPending, boardQ.isPending, layoutQ.isError, boardQ.isError]);

  if (layoutQ.isPending || boardQ.isPending) return <DetailSkeleton />;
  if (layoutQ.isError) return <ErrorState error={layoutQ.error} onRetry={() => void layoutQ.refetch()} />;
  if (boardQ.isError) return <ErrorState error={boardQ.error} onRetry={() => void boardQ.refetch()} />;

  return (
    <div
      ref={containerRef}
      className="h-[600px] w-full overflow-hidden rounded-xl border border-steel-200"
    />
  );
}
