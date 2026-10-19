import { useEffect, useRef, useState } from 'react';
import { native, type CloudOptions, type CloudStatus } from '../native';
import { errorMessage } from '../util';
import { Icon } from '../Icon';

export function CloudBackup() {
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const live = useRef(false);
  const request = useRef(false);

  useEffect(() => {
    live.current = true;
    let polling = false;
    const refresh = async () => {
      if (polling || request.current) return;
      polling = true;
      try {
        const value = await native.cloudStatus();
        if (live.current && !request.current) { setStatus(value); setLoadError(null); }
      } catch (e) { if (live.current) setLoadError(errorMessage(e)); }
      finally { polling = false; }
    };
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, 2000);
    return () => { live.current = false; window.clearInterval(timer); };
  }, []);

  const run = async (operation: () => Promise<CloudStatus>) => {
    if (request.current) return;
    request.current = true; setPending(true); setError(null);
    try { const value = await operation(); if (live.current) setStatus(value); }
    catch (e) { if (live.current) setError(errorMessage(e)); }
    finally { request.current = false; if (live.current) setPending(false); }
  };
  const busy = pending || status?.busy === true;
  const patch = (options: Partial<CloudOptions>) => {
    if (status) void run(() => native.cloudOptions({ ...status.options, ...options }));
  };
  const act = (action: Parameters<typeof native.cloudAction>[0]) => { void run(() => native.cloudAction(action)); };
  const selected = status && (status.options.images || status.options.links || status.options.videos);
  const problem = error ?? loadError ?? status?.error;
  const message = !status ? 'Loading cloud backup…' : status.busy ? status.phase || 'Working…'
    : status.connected ? status.options.paused ? 'Backup paused' : !selected ? 'Choose what to back up' : problem ? 'Backup needs attention' : status.phase || 'Automatic backup is on'
    : status?.configured ? 'Ready to connect your Google account' : 'Google Drive is not connected';

  return <section aria-labelledby="s-cloud" className="cloud-backup">
    <h2 id="s-cloud">Cloud backup</h2>
    <div className="cloud-connection">
      <div className="cloud-heading">
        <span className="cloud-mark"><Icon name="cloud" /></span>
        <div className="cloud-identity"><strong>Google Drive</strong><span className="note cloud-account">{status?.account ?? 'Your account. Your storage.'}</span></div>
        <span className="cloud-badge">{status?.connected ? 'Connected' : 'Optional'}</span>
      </div>
      <div className="cloud-status" data-attention={!!problem}>
        <p role="status" aria-live="polite">{message}</p>
        {status?.lastBackup && <p className="note">Last backup check · <time dateTime={status.lastBackup}>{new Date(status.lastBackup).toLocaleString()}</time></p>}
        {status?.connected && !status.busy && <p className="note">{status.options.paused ? 'Uploads are paused. Restore is still available.' : 'Checks for selected items every minute while Captrain is open.'}</p>}
      </div>
      {status && <div className="row-actions cloud-actions">
        {status.connected ? <>
          <button className="btn accent" disabled={busy || status.options.paused || !selected} onClick={() => act('backup')}>Back up now</button>
          <button className="btn" disabled={busy} onClick={() => patch({ paused: !status.options.paused })}>{status.options.paused ? 'Resume backup' : 'Pause backup'}</button>
        </> : <button className="btn accent" disabled={busy || !status.configured} onClick={() => act('connect')}>Connect Google Drive</button>}
        {status.busy && <button className="btn" disabled={pending} onClick={() => act('cancel')}>Cancel operation</button>}
      </div>}
      {status?.busy && <p className="note">Cancel stops after the current request finishes.</p>}
      {problem && <p className="field-error" role="alert">{problem}</p>}
    </div>
    <div className="cloud-location">
      <strong>Private app storage · hidden from My Drive</strong>
      <p className="note">Backups use your Google storage. Originals stay on this PC.</p>
      <details><summary>Where are my backups?</summary><p className="note">Google Drive keeps them in Captrain’s hidden app-data folder, so they won’t appear in My Drive or search. Restore them below. To manage stored app data, open Google Drive → Settings → Manage apps → Captrain.</p></details>
    </div>
    {status && <>
      <fieldset className="cloud-types" disabled={busy}>
        <legend>What to back up & restore</legend>
        {([['images', 'Screenshots', 'image', 'Original images'], ['links', 'Web links', 'link', 'Saved URLs'], ['videos', 'Videos', 'video', 'Uses more storage']] as const).map(([key, label, icon, hint]) =>
          <label key={key} className="cloud-type" data-selected={status.options[key]}><Icon name={icon} /><span><strong>{label}</strong><small>{hint}</small></span><input type="checkbox" checked={status.options[key]} onChange={e => patch({ [key]: e.target.checked })} /></label>)}
      </fieldset>
      <label className="cloud-history"><span><strong>Include existing library items</strong><small>{status.options.includeExisting ? 'Existing and new selected items will upload.' : 'Older items are excluded. Only new selected items will upload.'}</small></span><input type="checkbox" disabled={busy} checked={status.options.includeExisting} onChange={e => patch({ includeExisting: e.target.checked })} /></label>
      <p className="note">Windows folder links and PC save paths stay on this device.</p>
      {status.connected && <>
        <div className="cloud-restore"><div><strong>Restore from Drive</strong><p className="note">Add missing items of the selected types. Existing items stay unchanged.</p></div><button className="btn" disabled={busy || !selected} onClick={() => act('restore')}>Restore missing items</button></div>
        <details className="cloud-disclosure"><summary>Account & backup options</summary><div className="cloud-detail-body"><p className="note">Deleting a local item keeps its cloud copy. Disconnecting keeps both local and cloud copies.</p><div className="row-actions"><button className="btn" disabled={busy} onClick={() => act('connect')}>Reconnect Google Drive</button><button className="btn" disabled={busy} onClick={() => act('disconnect')}>Disconnect account</button></div></div></details>
      </>}
    </>}
    {status && !status.connected && <details className="cloud-disclosure" open={!status.configured}><summary>Developer setup</summary>
      <div className="cloud-detail-body">
      <p className="note">This development copy needs Captrain’s Desktop OAuth configuration before sign-in.</p>
      <button className="btn" disabled={busy} onClick={() => { void run(native.cloudConfigure); }}>{status.configured ? 'Replace OAuth file…' : 'Import OAuth file…'}</button>
      <ol className="cloud-setup">
        <li>In Google Cloud Console, create or select a project and enable the Google Drive API.</li>
        <li>Configure Google Auth Platform consent. While testing, add your Google account as a test user.</li>
        <li>Create an OAuth client with application type Desktop app. Download its JSON and import it above.</li>
        <li>Choose what to back up, then connect Google Drive in your browser. A testing app’s authorization can expire after seven days.</li>
      </ol>
      <p className="note">Use the same OAuth app on your other PCs to access these backups. Public releases need a production OAuth app. The imported configuration and sign-in credentials are encrypted for this Windows user; do not share or commit the downloaded file.</p>
      </div>
    </details>}
  </section>;
}
