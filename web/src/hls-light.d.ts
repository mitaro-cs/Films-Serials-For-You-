// Облегчённая сборка hls.js (без субтитров и DRM — они нам не нужны) с теми же типами.
declare module 'hls.js/light' {
  export * from 'hls.js';
  export { default } from 'hls.js';
}
