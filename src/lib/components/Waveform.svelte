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

  /** Linear interpolation; `t` in [0,1] across the full envelope. */
  function sampleEnvelope(env: Float32Array, t: number): number {
    const n = env.length;
    if (n === 0) return 0;
    if (n === 1) return env[0];
    const x = Math.max(0, Math.min(1, t)) * (n - 1);
    const i0 = Math.floor(x);
    const i1 = Math.min(n - 1, i0 + 1);
    const f = x - i0;
    return env[i0] * (1 - f) + env[i1] * f;
  }

  function timeFromClientX(clientX: number, el: HTMLCanvasElement): number {
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return ratio * duration;
  }

  function onPointerDown(e: PointerEvent) {
    if (!seekable || e.button !== 0) return;
    const c = canvas;
    if (!c) return;
    if (e.shiftKey && clipSelectable) {
      try {
        c.setPointerCapture(e.pointerId);
      } catch {
        /* capture may fail */
      }
      selectPointerId = e.pointerId;
      selectAnchor = timeFromClientX(e.clientX, c);
      selectCurrent = selectAnchor;
      return;
    }
    try {
      c.setPointerCapture(e.pointerId);
    } catch {
      /* capture may fail in edge cases */
    }
    scrubPointerId = e.pointerId;
    scrubTime = timeFromClientX(e.clientX, c);
  }

  function onPointerMove(e: PointerEvent) {
    const c = canvas;
    if (!c) return;
    if (e.pointerId === selectPointerId && selectAnchor != null) {
      selectCurrent = timeFromClientX(e.clientX, c);
      return;
    }
    if (e.pointerId !== scrubPointerId) return;
    scrubTime = timeFromClientX(e.clientX, c);
  }

  function endSelect(e: PointerEvent) {
    if (e.pointerId !== selectPointerId) return;
    const c = canvas;
    if (c) {
      try {
        c.releasePointerCapture(e.pointerId);
      } catch {
        /* already released */
      }
    }
    const a = selectAnchor;
    const b = selectCurrent;
    selectPointerId = null;
    selectAnchor = null;
    selectCurrent = null;
    if (a == null || b == null || !onClipChange) return;
    const lo = Math.min(a, b);
    const hi = Math.max(a, b);
    if (hi - lo < CLIP_MIN_SEC) return;
    void Promise.resolve(onClipChange(lo, hi));
  }

  function endScrub(e: PointerEvent) {
    if (e.pointerId !== scrubPointerId) return;
    const c = canvas;
    if (c) {
      try {
        c.releasePointerCapture(e.pointerId);
      } catch {
        /* already released */
      }
    }
    const t = scrubTime;
    scrubPointerId = null;
    scrubTime = null;
    if (t != null && seekable && onSeek) {
      onSeek(Math.max(0, Math.min(duration, t)));
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (e.pointerId === selectPointerId) {
      endSelect(e);
      return;
    }
    endScrub(e);
  }

  function onPointerCancel(e: PointerEvent) {
    endSelect(e);
    endScrub(e);
  }

  function draw() {
    const c = canvas;
    const wrap = wrapEl;
    if (!c || !wrap) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(2.5, typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1);
    const cssW = Math.max(280, Math.floor(wrap.clientWidth));
    const cssH = 80;
    c.width = Math.floor(cssW * dpr);
    c.height = Math.floor(cssH * dpr);
    c.style.width = `${cssW}px`;
    c.style.height = `${cssH}px`;

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
          const t = (px + 0.5) / w;
          const v = sampleEnvelope(env, t);
          const bh = v * (h * 0.45);
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

      const t = scrubTime ?? currentTime;
      const x = (t / duration) * w;
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
