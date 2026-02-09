import {
  type CSSProperties,
  type DependencyList,
  type PointerEvent,
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type WheelEvent,
} from "react";

type ZoomDirection = "in" | "out";

interface UseZoomPanOptions {
  minScale?: number;
  maxScale?: number;
  zoomIntensity?: number;
  zoomStep?: number;
  wheelSessionGapMs?: number;
  resetDeps?: DependencyList;
  panExcludeSelector?: string;
}

interface UseZoomPanResult {
  viewportRef: RefObject<HTMLDivElement | null>;
  transformStyle: CSSProperties;
  cursorClassName: string;
  isPanning: boolean;
  scale: number;
  zoomPercent: number;
  canZoomIn: boolean;
  canZoomOut: boolean;
  handleWheel: (event: WheelEvent<HTMLDivElement>) => void;
  handlePointerDown: (event: PointerEvent<HTMLDivElement>) => void;
  handlePointerMove: (event: PointerEvent<HTMLDivElement>) => void;
  endPan: () => void;
  zoomAtCenter: (direction: ZoomDirection) => void;
}

export function useZoomPan(options: UseZoomPanOptions = {}): UseZoomPanResult {
  const {
    minScale = 1,
    maxScale = 8,
    zoomIntensity = 0.0015,
    zoomStep = 1.2,
    wheelSessionGapMs = 240,
    resetDeps = [],
    panExcludeSelector,
  } = options;

  const viewportRef = useRef<HTMLDivElement>(null);
  const wheelSessionRef = useRef<{
    lastTime: number;
    anchorScreen: { x: number; y: number };
    anchorContent: { x: number; y: number };
  } | null>(null);
  const panStartRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    startOffset: { x: number; y: number };
  } | null>(null);
  const transformRef = useRef({ scale: minScale, offset: { x: 0, y: 0 } });

  const [scale, setScale] = useState(minScale);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);

  const clampScale = useCallback(
    (value: number) => Math.min(maxScale, Math.max(minScale, value)),
    [maxScale, minScale]
  );

  const clampOffset = useCallback((nextOffset: { x: number; y: number }, nextScale: number) => {
    const viewport = viewportRef.current;
    if (!viewport) return nextOffset;
    const rect = viewport.getBoundingClientRect();
    if (!rect.width || !rect.height) return nextOffset;
    const minX = rect.width * (1 - nextScale);
    const minY = rect.height * (1 - nextScale);
    return {
      x: Math.min(0, Math.max(minX, nextOffset.x)),
      y: Math.min(0, Math.max(minY, nextOffset.y)),
    };
  }, []);

  const applyZoomAtPoint = useCallback(
    (
      nextScale: number,
      anchorScreen: { x: number; y: number },
      anchorContent: { x: number; y: number }
    ) => {
      const clampedScale = clampScale(nextScale);
      const nextOffset = {
        x: anchorScreen.x - anchorContent.x * clampedScale,
        y: anchorScreen.y - anchorContent.y * clampedScale,
      };
      const clampedOffset = clampOffset(nextOffset, clampedScale);
      setScale(clampedScale);
      setOffset(clampedOffset);
    },
    [clampOffset, clampScale]
  );

  const handleWheel = useCallback(
    (event: WheelEvent<HTMLDivElement>) => {
      const viewport = viewportRef.current;
      if (!viewport) return;
      event.preventDefault();
      const transform = transformRef.current;

      const now = performance.now();
      const rect = viewport.getBoundingClientRect();
      const screenPoint = {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      };

      if (!wheelSessionRef.current || now - wheelSessionRef.current.lastTime > wheelSessionGapMs) {
        wheelSessionRef.current = {
          lastTime: now,
          anchorScreen: screenPoint,
          anchorContent: {
            x: (screenPoint.x - transform.offset.x) / transform.scale,
            y: (screenPoint.y - transform.offset.y) / transform.scale,
          },
        };
      } else {
        wheelSessionRef.current.lastTime = now;
      }

      const session = wheelSessionRef.current;
      const zoomFactor = Math.exp(-event.deltaY * zoomIntensity);
      applyZoomAtPoint(transform.scale * zoomFactor, session.anchorScreen, session.anchorContent);
    },
    [applyZoomAtPoint, wheelSessionGapMs, zoomIntensity]
  );

  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const shouldPan = scale > minScale + 0.001;
      if (!shouldPan || event.button !== 0) return;
      if (panExcludeSelector) {
        const target = event.target;
        if (target instanceof Element && target.closest(panExcludeSelector)) {
          return;
        }
      }
      const viewport = viewportRef.current;
      if (!viewport) return;
      event.preventDefault();
      event.stopPropagation();
      viewport.setPointerCapture(event.pointerId);
      panStartRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        startOffset: transformRef.current.offset,
      };
      setIsPanning(true);
    },
    [minScale, panExcludeSelector, scale]
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const pan = panStartRef.current;
      if (event.pointerId !== pan?.pointerId) return;
      event.preventDefault();
      const dx = event.clientX - pan.startX;
      const dy = event.clientY - pan.startY;
      const nextOffset = { x: pan.startOffset.x + dx, y: pan.startOffset.y + dy };
      setOffset(clampOffset(nextOffset, transformRef.current.scale));
    },
    [clampOffset]
  );

  const endPan = useCallback(() => {
    const viewport = viewportRef.current;
    const pan = panStartRef.current;
    if (pan && viewport) {
      try {
        viewport.releasePointerCapture(pan.pointerId);
      } catch {
        // ignore
      }
    }
    panStartRef.current = null;
    setIsPanning(false);
  }, []);

  const zoomAtCenter = useCallback(
    (direction: ZoomDirection) => {
      const viewport = viewportRef.current;
      if (!viewport) return;
      const rect = viewport.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const screenPoint = { x: rect.width / 2, y: rect.height / 2 };
      const transform = transformRef.current;
      const anchorContent = {
        x: (screenPoint.x - transform.offset.x) / transform.scale,
        y: (screenPoint.y - transform.offset.y) / transform.scale,
      };
      const targetScale =
        direction === "in" ? transform.scale * zoomStep : transform.scale / zoomStep;
      applyZoomAtPoint(targetScale, screenPoint, anchorContent);
    },
    [applyZoomAtPoint, zoomStep]
  );

  useEffect(() => {
    transformRef.current = { scale, offset };
  }, [scale, offset]);

  useEffect(() => {
    setScale(minScale);
    setOffset({ x: 0, y: 0 });
    wheelSessionRef.current = null;
  }, [minScale, ...resetDeps]);

  useEffect(() => {
    setOffset((prev) => clampOffset(prev, scale));
  }, [clampOffset, scale]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      setOffset((prev) => clampOffset(prev, transformRef.current.scale));
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [clampOffset]);

  const zoomPercent = useMemo(() => Math.round(scale * 100), [scale]);
  const canZoomIn = scale < maxScale - 0.001;
  const canZoomOut = scale > minScale + 0.001;
  const cursorClassName = isPanning
    ? "cursor-grabbing"
    : scale > minScale + 0.001
      ? "cursor-grab"
      : "cursor-default";

  const transformStyle = useMemo(
    () => ({
      transform: `translate(${String(offset.x)}px, ${String(offset.y)}px) scale(${String(scale)})`,
      transformOrigin: "0 0",
    }),
    [offset.x, offset.y, scale]
  );

  return {
    viewportRef,
    transformStyle,
    cursorClassName,
    isPanning,
    scale,
    zoomPercent,
    canZoomIn,
    canZoomOut,
    handleWheel,
    handlePointerDown,
    handlePointerMove,
    endPan,
    zoomAtCenter,
  };
}
