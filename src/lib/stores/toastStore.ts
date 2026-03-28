import { writable } from "svelte/store";

export type ToastType = "error" | "success" | "info";

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  timeout: number;
}

function id() {
  return `t-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

function createToastStore() {
  const { subscribe, update } = writable<{ toasts: Toast[] }>({ toasts: [] });

  return {
    subscribe,
    show(message: string, type: ToastType = "info", timeout = 4000) {
      const t: Toast = { id: id(), message, type, timeout };
      update((s) => ({ toasts: [...s.toasts, t] }));
      if (timeout > 0) {
        setTimeout(() => {
          update((s) => ({ toasts: s.toasts.filter((x) => x.id !== t.id) }));
        }, timeout);
      }
    },
    dismiss(tid: string) {
      update((s) => ({ toasts: s.toasts.filter((x) => x.id !== tid) }));
    },
  };
}

export const toastStore = createToastStore();
