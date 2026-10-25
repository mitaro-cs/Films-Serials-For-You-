/** Keep a heading on one line; ellipsis only for titles too long even at 16px. */
export function fitTitle(node: HTMLElement, text: string) {
  let alive = true;
  let width = -1;
  function fit() {
    if (!alive) return;
    node.style.fontSize = '';
    const max = parseFloat(getComputedStyle(node).fontSize);
    if (node.scrollWidth > node.clientWidth && node.clientWidth > 0) {
      node.style.fontSize = `${Math.max(16, Math.floor(max * node.clientWidth / node.scrollWidth))}px`;
    }
  }
  const observer = new ResizeObserver(() => {
    const next = node.clientWidth;
    if (next !== width) { width = next; fit(); }
  });
  observer.observe(node);
  void document.fonts.ready.then(fit);
  fit();
  return {
    update(next: string) { if (next !== text) { text = next; requestAnimationFrame(fit); } },
    destroy() { alive = false; observer.disconnect(); },
  };
}
