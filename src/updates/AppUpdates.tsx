import { useState } from 'react';
import { updates, useUpdates } from './updates';
import { version } from '../../package.json';

export function AppUpdates() {
  const update = useUpdates();
  const [showNotes, setShowNotes] = useState(false);
  const busy = ['unsupported', 'checking', 'downloading', 'installing'].includes(update.status);
  const label = update.status === 'available' ? 'Download update'
    : update.status === 'downloaded' ? 'Restart and install'
    : update.status === 'checking' ? 'Checking…'
    : update.status === 'downloading' ? 'Downloading…'
    : update.status === 'installing' ? 'Restarting…' : 'Check for updates';
  const description = update.error ?? (
    update.status === 'unsupported' ? 'Updates are available in the installed Windows x64 app.'
    : update.status === 'up-to-date' ? 'You’re up to date.'
    : update.status === 'available' ? `Version ${update.available?.version} is available.`
    : update.status === 'downloaded' ? 'Ready to install. Finish your capture and close the viewer before restarting.'
    : update.status === 'installing' ? 'Restarting Captrain to install the update…'
    : update.status === 'checking' ? 'Checking GitHub for updates…'
    : update.status === 'downloading' ? 'Downloading the signed update. You can keep working.'
    : 'Checks on launch and every six hours. Download and restart when you’re ready.');
  return (
    <section aria-labelledby="s-updates">
      <h2 id="s-updates">App updates</h2>
      <p className="note">Version {version}</p>
      <p role="status">{description}</p>
      {update.status === 'downloading' && <progress aria-label="Update download" max={100} value={update.percent ?? undefined} />}
      <div className="row-actions">
        <button className="btn" disabled={busy} onClick={() => { void (update.status === 'available' ? updates.download() : update.status === 'downloaded' ? updates.install() : updates.check()); }}>{label}</button>
        {update.available?.notes && <button className="btn" aria-expanded={showNotes} onClick={() => setShowNotes(x => !x)}>Release notes</button>}
      </div>
      {showNotes && update.available?.notes && <pre className="update-notes">{update.available.notes}</pre>}
    </section>
  );
}
