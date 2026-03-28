<script lang="ts">
  import { toastStore } from "$lib/stores/toastStore";
</script>

<div class="toasts" aria-live="polite">
  {#each $toastStore.toasts as t (t.id)}
    <div class="toast {t.type}" role="status">
      <span>{t.message}</span>
      <button type="button" class="dismiss" onclick={() => toastStore.dismiss(t.id)} aria-label="Dismiss">×</button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    top: var(--spacing-md);
    right: var(--spacing-md);
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    max-width: 360px;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    font-size: var(--font-size-sm);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }
  .toast.error {
    border-color: var(--error);
  }
  .toast.success {
    border-color: var(--success);
  }
  .dismiss {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
  }
  .dismiss:hover {
    color: var(--text-primary);
  }
</style>
