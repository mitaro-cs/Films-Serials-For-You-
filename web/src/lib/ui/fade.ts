// Картинка появляется плавно, когда загрузится (а не «выскакивает» кусками). Уже загруженная
// (из кеша) - сразу, без мигания. Только opacity - видеокарте легко.
export function fadeIn(img: HTMLImageElement) {
  if (img.complete && img.naturalWidth) return;
  img.style.opacity = '0';
  img.style.transition = 'opacity 0.45s ease';
  const show = () => (img.style.opacity = '1');
  img.addEventListener('load', show, { once: true });
  img.addEventListener('error', show, { once: true });
}
