export interface Available { version: string; notes: string | null }
export type Status = 'unsupported' | 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'downloaded' | 'installing' | 'error';
export interface UpdateState {
  status: Status;
  available: Available | null;
  percent: number | null;
  error: string | null;
}

export function createUpdates(api: {
  check(): Promise<Available | null>;
  download(progress: (percent: number | null) => void): Promise<void>;
  install(): Promise<void>;
}, supported: boolean) {
  let state: UpdateState = { status: supported ? 'idle' : 'unsupported', available: null, percent: null, error: null };
  let pending: Promise<void> | undefined;
  const listeners = new Set<() => void>();
  const publish = (patch: Partial<UpdateState>) => {
    state = { ...state, ...patch };
    listeners.forEach(fn => fn());
  };
  const exclusive = (action: () => Promise<void>) => {
    if (pending) return pending;
    pending = Promise.resolve().then(action).finally(() => { pending = undefined; });
    return pending;
  };
  const fail = (error: unknown, status: Status) => publish({ status, percent: null, error: error instanceof Error ? error.message : String(error) });
  return {
    getState: () => state,
    subscribe: (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; },
    check() {
      if (!supported || ['downloading', 'downloaded', 'installing'].includes(state.status)) return Promise.resolve();
      return exclusive(async () => {
        publish({ status: 'checking', error: null, percent: null });
        try {
          const available = await api.check();
          publish({ available, status: available ? 'available' : 'up-to-date' });
        } catch (e) { fail(e, 'error'); }
      });
    },
    download() {
      if (state.status !== 'available') return pending ?? Promise.resolve();
      return exclusive(async () => {
        publish({ status: 'downloading', percent: null, error: null });
        try {
          await api.download(percent => publish({ percent: percent !== null && Number.isFinite(percent) ? Math.max(0, Math.min(100, percent)) : null }));
          publish({ status: 'downloaded', percent: 100 });
        } catch (e) { fail(e, 'available'); }
      });
    },
    install() {
      if (state.status !== 'downloaded') return pending ?? Promise.resolve();
      return exclusive(async () => {
        publish({ status: 'installing', error: null });
        try { await api.install(); }
        catch (e) { fail(e, 'downloaded'); }
      });
    },
  };
}
