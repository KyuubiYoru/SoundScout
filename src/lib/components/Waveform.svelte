<script lang="ts">
  import { onMount } from "svelte";
  import { CLIP_MIN_SEC } from "$lib/stores/playerStore";

  /** Extra time shown on each side of the clip when preview-zoomed (fraction of clip length, with a floor). */
  const PREVIEW_ZOOM_PAD_FRAC = 0.12;
  const PREVIEW_ZOOM_PAD_MIN_SEC = 0.08;

  let {
    peaks,
    currentTime = 0,
    duration = 0,
    clipRange = null,
    /** When true (e.g. export preview with a clip), map the waveform X axis to the clip only. */
    zoomToClipPreview = false,
    onSeek,
    onClipChange,
  }: {
    peaks: number[];
    currentTime?: number;
    duration?: number;
    clipRange?: { start: number; end: number } | null;
    zoomToClipPreview?: boolean;
    onSeek?: (seconds: number) => void;
    onClipChange?: (start: number, end: number) => void | Promise<void>;
  } = $props();

  /** In preview zoom mode, timeline seconds mapped to full canvas width (clip ± padding). */
  const viewWindow = $derived.by(() => {
    if (!zoomToClipPreview || !clipRange || duration <= 0) return null;
    const clipLen = Math.max(CLIP_MIN_SEC, clipRange.end - clipRange.start);
    const pad = Math.max(PREVIEW_ZOOM_PAD_MIN_SEC, clipLen * PREVIEW_ZOOM_PAD_FRAC);
    const start = Math.max(0, clipRange.start - pad);
    const end = Math.min(duration, clipRange.end + pad);
    const span = Math.max(CLIP_MIN_SEC, end - start);
    return { start, span };
  });

  let canvas = $state<HTMLCanvasElement | null>(null);
  let wrapEl = $state<HTMLDivElement | null>(null);
  /** Local preview time while dragging; committed on pointerup. */
  let scrubTime = $state<number | null>(null);
  let scrubPointerId = $state<number | null>(null);
  /** Shift+drag clip selection */
  let selectPointerId = $state<number | null>(null);
  let selectAnchor = $state<number | null>(null);
  let selectCurrent = $state<number | null>(null);

  const HANDLE_HIT_PX = 8;
  let edgeDragEdge = $state<"start" | "end" | null>(null);
  let edgeDragPointerId = $state<number | null>(null);
  let edgeDragTime = $state<number | null>(null);

  const seekable = $derived(duration > 0 && typeof onSeek === "function");
  const clipSelectable = $derived(
    duration > 0 && typeof onClipChange === "function",
  );

  /** Indexer stores min/max pairs per bucket; collapse to one envelope height per bucket. */
  function peakEnvelope(peaks: number[]): Float32Array {
    const pairs = Math.floor(peaks.length / 2);
    const out = new Float32Array(pairs);
    for (let b = 0; b < pairs; b++) {
      const mn = peaks[b * 2] ?? 0;
      const mx = peaks[b * 2 + 1] ?? 0;
      out[b] = Math.min(1, Math.max(Math.abs(mn), Math.abs(mx)));
    }
    return out;
  }

  /** Linear interpolation; `normalizedT` in [0,1] across the full envelope. */
  function sampleEnvelope(env: Float32Array, normalizedT: number): number {
    const envLen = env.length;
    if (envLen === 0) return 0;
    if (envLen === 1) return env[0];
    const x = Math.max(0, Math.min(1, normalizedT)) * (envLen - 1);
    const i0 = Math.floor(x);
    const i1 = Math.min(envLen - 1, i0 + 1);
    const blendFactor = x - i0;
    return env[i0] * (1 - blendFactor) + env[i1] * blendFactor;
  }

  function timeFromClientX(clientX: number, el: HTMLCanvasElement): number {
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    const vw = viewWindow;
    if (vw) return vw.start + ratio * vw.span;
    return ratio * duration;
  }

  function getEdgeAtX(
    clientX: number,
    canvasEl: HTMLCanvasElement,
  ): "start" | "end" | null {
    if (!clipRange || duration <= 0) return null;
    const rect = canvasEl.getBoundingClientRect();
    if (rect.width <= 0) return null;
    const vw = viewWindow;
    const span = vw ? vw.span : duration;
    const cx = clientX - rect.left;
    const xAt = (t: number) => (vw ? ((t - vw.start) / span) * rect.width : (t / duration) * rect.width);
    if (Math.abs(cx - xAt(clipRange.start)) <= HANDLE_HIT_PX) return "start";
    if (Math.abs(cx - xAt(clipRange.end)) <= HANDLE_HIT_PX) return "end";
    return null;
  }

  function onPointerDown(event: PointerEvent) {
    if (!seekable || event.button !== 0) return;
    const canvasEl = canvas;
    if (!canvasEl) return;

    if (clipSelectable && !event.shiftKey) {
      const edge = getEdgeAtX(event.clientX, canvasEl);
      if (edge && clipRange) {
        try {
          canvasEl.setPointerCapture(event.pointerId);
        } catch {
          /* capture may fail */
        }
        edgeDragEdge = edge;
        edgeDragPointerId = event.pointerId;
        edgeDragTime = edge === "start" ? clipRange.start : clipRange.end;
        return;
      }
    }

    if (event.shiftKey && clipSelectable) {
      try {
        canvasEl.setPointerCapture(event.pointerId);
      } catch {
        /* capture may fail */
      }
      selectPointerId = event.pointerId;
      selectAnchor = timeFromClientX(event.clientX, canvasEl);
      selectCurrent = selectAnchor;
      return;
    }
    try {
      canvasEl.setPointerCapture(event.pointerId);
    } catch {
      /* capture may fail in edge cases */
    }
    scrubPointerId = event.pointerId;
    scrubTime = timeFromClientX(event.clientX, canvasEl);
  }

  function onPointerMove(event: PointerEvent) {
    const canvasEl = canvas;
    if (!canvasEl) return;
    if (event.pointerId === edgeDragPointerId && edgeDragEdge != null) {
      edgeDragTime = timeFromClientX(event.clientX, canvasEl);
      canvasEl.style.cursor = "ew-resize";
      return;
    }
    if (event.pointerId === selectPointerId && selectAnchor != null) {
      selectCurrent = timeFromClientX(event.clientX, canvasEl);
      return;
    }
    if (event.pointerId === scrubPointerId) {
      scrubTime = timeFromClientX(event.clientX, canvasEl);
      return;
    }
    if (
      edgeDragPointerId == null &&
      selectPointerId == null &&
      scrubPointerId == null &&
      canvas &&
      clipSelectable &&
      clipRange
    ) {
      const edge = getEdgeAtX(event.clientX, canvas);
      canvas.style.cursor = edge ? "ew-resize" : "";
    }
  }

  function endEdgeDrag(event: PointerEvent) {
    if (event.pointerId !== edgeDragPointerId) return;
    const canvasEl = canvas;
    if (canvasEl) {
      try {
        canvasEl.releasePointerCapture(event.pointerId);
      } catch {
        /* already released */
      }
      canvasEl.style.cursor = "";
    }
    const edge = edgeDragEdge;
    const t = edgeDragTime;
    const cr = clipRange;
    edgeDragEdge = null;
    edgeDragPointerId = null;
    edgeDragTime = null;
    if (edge == null || t == null || cr == null || !onClipChange) return;
    const start = edge === "start" ? t : cr.start;
    const end = edge === "end" ? t : cr.end;
    void Promise.resolve(onClipChange(start, end));
  }

  function cancelEdgeDrag(event: PointerEvent) {
    if (event.pointerId !== edgeDragPointerId) return;
    const canvasEl = canvas;
    if (canvasEl) {
      try {
        canvasEl.releasePointerCapture(event.pointerId);
      } catch {
        /* already released */
      }
      canvasEl.style.cursor = "";
    }
    edgeDragEdge = null;
    edgeDragPointerId = null;
    edgeDragTime = null;
  }

  function endSelect(event: PointerEvent) {
    if (event.pointerId !== selectPointerId) return;
    const canvasEl = canvas;
    if (canvasEl) {
      try {
        canvasEl.releasePointerCapture(event.pointerId);
      } catch {
        /* already released */
      }
    }
    const anchorSec = selectAnchor;
    const currentSec = selectCurrent;
    selectPointerId = null;
    selectAnchor = null;
    selectCurrent = null;
    if (anchorSec == null || currentSec == null || !onClipChange) return;
    const lo = Math.min(anchorSec, currentSec);
    const hi = Math.max(anchorSec, currentSec);
    if (hi - lo < CLIP_MIN_SEC) return;
    void Promise.resolve(onClipChange(lo, hi));
  }

  function endScrub(event: PointerEvent) {
    if (event.pointerId !== scrubPointerId) return;
    const canvasEl = canvas;
    if (canvasEl) {
      try {
        canvasEl.releasePointerCapture(event.pointerId);
      } catch {
        /* already released */
      }
    }
    const committedTime = scrubTime;
    scrubPointerId = null;
    scrubTime = null;
    if (committedTime != null && seekable && onSeek) {
      onSeek(Math.max(0, Math.min(duration, committedTime)));
    }
  }

  function onPointerUp(event: PointerEvent) {
    if (event.pointerId === edgeDragPointerId) {
      endEdgeDrag(event);
      return;
    }
    if (event.pointerId === selectPointerId) {
      endSelect(event);
      return;
    }
    endScrub(event);
  }

  function onPointerCancel(event: PointerEvent) {
    cancelEdgeDrag(event);
    endSelect(event);
    endScrub(event);
  }

  function draw() {
    const canvasEl = canvas;
    const wrap = wrapEl;
    if (!canvasEl || !wrap) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(2.5, typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1);
    const cssW = Math.max(280, Math.floor(wrap.clientWidth));
    const cssH = 80;
    canvasEl.width = Math.floor(cssW * dpr);
    canvasEl.height = Math.floor(cssH * dpr);
    canvasEl.style.width = `${cssW}px`;
    canvasEl.style.height = `${cssH}px`;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const w = cssW;
    const h = cssH;
    ctx.fillStyle = "#1a1e26";
    ctx.fillRect(0, 0, w, h);

    const vw = viewWindow;

    if (peaks.length) {
      const env = peakEnvelope(peaks);
      if (env.length > 0) {
        const mid = h / 2;
        ctx.fillStyle = "rgba(74, 144, 217, 0.45)";
        for (let px = 0; px < w; px++) {
          const tCenter = vw
            ? vw.start + ((px + 0.5) / w) * vw.span
            : ((px + 0.5) / w) * duration;
          const phaseT = duration > 0 ? tCenter / duration : 0;
          const envelopeAmp = sampleEnvelope(env, phaseT);
          const bh = envelopeAmp * (h * 0.45);
          ctx.fillRect(px, mid - bh, 1, bh * 2);
        }
      }
    }

    if (duration > 0) {
      const xAt = (t: number) => {
        if (vw) return ((t - vw.start) / vw.span) * w;
        return (t / duration) * w;
      };

      const drawRange = (t0: number, t1: number, fill: string) => {
        const x0 = xAt(t0);
        const x1 = xAt(t1);
        const left = Math.min(x0, x1);
        const rw = Math.max(1, Math.abs(x1 - x0));
        ctx.fillStyle = fill;
        ctx.fillRect(left, 0, rw, h);
      };

      let liveClipStart = 0;
      let liveClipEnd = 0;
      if (clipRange) {
        liveClipStart =
          edgeDragEdge === "start" && edgeDragTime != null ? edgeDragTime : clipRange.start;
        liveClipEnd =
          edgeDragEdge === "end" && edgeDragTime != null ? edgeDragTime : clipRange.end;
        const xClip0 = xAt(liveClipStart);
        const xClip1 = xAt(liveClipEnd);
        const dimLeft = Math.min(xClip0, xClip1);
        const dimRight = Math.max(xClip0, xClip1);
        ctx.fillStyle = "rgba(0,0,0,0.45)";
        ctx.fillRect(0, 0, dimLeft, h);
        ctx.fillRect(dimRight, 0, w - dimRight, h);
        drawRange(liveClipStart, liveClipEnd, "rgba(74, 144, 217, 0.22)");
      }
      if (selectAnchor != null && selectCurrent != null) {
        drawRange(selectAnchor, selectCurrent, "rgba(120, 186, 255, 0.28)");
      }

      const playheadT = scrubTime ?? currentTime;
      let x = xAt(playheadT);
      x = Math.max(0, Math.min(w, x));
      ctx.strokeStyle = "rgba(226, 228, 232, 0.85)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();

      if (clipRange) {
        const accent = "#4a90d9";
        const drawHandle = (t: number, isStart: boolean) => {
          let xh = xAt(t);
          xh = Math.max(0, Math.min(w, xh));
          ctx.strokeStyle = accent;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.moveTo(xh, 0);
          ctx.lineTo(xh, h);
          ctx.stroke();
          const mid = h / 2;
          const dir = isStart ? 1 : -1;
          ctx.fillStyle = accent;
          ctx.beginPath();
          ctx.moveTo(xh, mid - 6);
          ctx.lineTo(xh + dir * 8, mid);
          ctx.lineTo(xh, mid + 6);
          ctx.closePath();
          ctx.fill();
        };
        drawHandle(liveClipStart, true);
        drawHandle(liveClipEnd, false);
      }
    }
  }

  onMount(() => {
    const wrap = wrapEl;
    if (!wrap || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => draw());
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  $effect(() => {
    peaks;
    currentTime;
    duration;
    scrubTime;
    clipRange;
    zoomToClipPreview;
    viewWindow;
    selectAnchor;
    selectCurrent;
    edgeDragTime;
    edgeDragEdge;
    wrapEl?.clientWidth;
    draw();
  });
</script>

<div class="wf-wrap" bind:this={wrapEl}>
  <canvas
    bind:this={canvas}
    class="wf"
    class:seekable
    class:clip-selectable={clipSelectable}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerCancel}
    onpointerleave={() => { if (edgeDragPointerId == null && canvas) canvas.style.cursor = ""; }}
  ></canvas>
</div>

<style>
  .wf-wrap {
    width: 100%;
    min-width: 0;
  }
  .wf {
    display: block;
    border-radius: 4px;
  }
  .wf.seekable {
    cursor: pointer;
    touch-action: none;
  }
  .wf.clip-selectable {
    touch-action: none;
  }
</style>
