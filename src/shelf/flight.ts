/**
 * Flies a decoded thumbnail from the middle of the panel into its row.
 * The real thumbnail stays hidden until it lands. Returns a cancel function.
 */
export function fly(src: string, from: DOMRect, target: HTMLElement): () => void {
  const to = target.getBoundingClientRect();
  const img = document.createElement('img');
  img.src = src;
  img.alt = '';
  img.className = 'fly';
  Object.assign(img.style, { left: `${to.left}px`, top: `${to.top}px`, width: `${to.width}px`, height: `${to.height}px` });
  const dx = from.left + from.width / 2 - (to.left + to.width / 2);
  const dy = from.top + from.height / 2 - (to.top + to.height / 2);
  const scale = Math.max(1, Math.min(3.4, 240 / to.width));
  target.style.visibility = 'hidden';
  document.body.append(img);
  const animation = img.animate([
    { transform: `translate(${dx}px, ${dy}px) scale(${scale})`, opacity: 0 },
    { opacity: 1, offset: 0.18 },
    { transform: 'translate(0, 0) scale(1)', opacity: 1 },
  ], { duration: 520, easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)' });
  const done = () => { img.remove(); target.style.visibility = ''; };
  animation.onfinish = done;
  animation.oncancel = done;
  return () => animation.cancel();
}
