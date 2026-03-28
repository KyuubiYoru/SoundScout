<script lang="ts">
  import { onMount } from "svelte";
  import { CLIP_MIN_SEC } from "$lib/stores/playerStore";

  let {
    peaks,
    currentTime = 0,
    duration = 0,
    clipRange = null,
    onSeek,
    onClipChange,
  }: {
    peaks: number[];
    currentTime?: number;
    duration?: number;
    clipRange?: { start: number; end: number } | null;
    onSeek?: (seconds: number) => void;
    onClipChange?: (start: number, end: number) => void | Promise<void>;
  } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let wrapEl = $state<HTMLDivElement | null>(null);
  /** Local preview time while dragging; committed on pointerup. */
  let scrubTime = $state<number | null>(null);
  let scrubPointerId = $state<number | null>(null);
  /** Shift+drag clip selection */
  let selectPointerId = $state<number | null>(null);
  let selectAnchor = $state<number | null>(null);
  let selectCurrent = $state<number | null>(null);

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
    return ratio * duration;
  }

  function onPointerDown(event: PointerEvent) {
    if (!seekable || event.button !== 0) return;
    const canvasEl = canvas;
    if (!canvasEl) return;
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
    if (event.pointerId === selectPointerId && selectAnchor != null) {
      selectCurrent = timeFromClientX(event.clientX, canvasEl);
      return;
    }
    if (event.pointerId !== scrubPointerId) return;
    scrubTime = timeFromClientX(event.clientX, canvasEl);
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
    if (event.pointerId === selectPointerId) {
      endSelect(event);
      return;
    }
    endScrub(event);
  }

  function onPointerCancel(event: PointerEvent) {
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

    if (peaks.length) {
      const env = peakEnvelope(peaks);
      if (env.length > 0) {
        const mid = h / 2;
        ctx.fillStyle = "rgba(74, 144, 217, 0.45)";
        for (let px = 0; px < w; px++) {
          const phaseT = (px + 0.5) / w;
          const envelopeAmp = sampleEnvelope(env, phaseT);
          const bh = envelopeAmp * (h * 0.45);
          ctx.fillRect(px, mid - bh, 1, bh * 2);
        }
      }
    }

    if (duration > 0) {
      const drawRange = (t0: number, t1: number, fill: string) => {
        const x0 = (t0 / duration) * w;
        const x1 = (t1 / duration) * w;
        const left = Math.min(x0, x1);
        const rw = Math.max(1, Math.abs(x1 - x0));
        ctx.fillStyle = fill;
        ctx.fillRect(left, 0, rw, h);
      };

      if (clipRange) {
        drawRange(clipRange.start, clipRange.end, "rgba(74, 144, 217, 0.22)");
      }
      if (selectAnchor != null && selectCurrent != null) {
        drawRange(selectAnchor, selectCurrent, "rgba(120, 186, 255, 0.28)");
      }

      const playheadT = scrubTime ?? currentTime;
      const x = (playheadT / duration) * w;
      ctx.strokeStyle = "rgba(226, 228, 232, 0.85)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();
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
    selectAnchor;
    selectCurrent;
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
    cursor: pointer;
  }
</style>
