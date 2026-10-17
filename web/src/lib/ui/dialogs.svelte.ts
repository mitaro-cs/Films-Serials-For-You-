// Свои окна вопросов: confirm() в окне приложения на Mac (WKWebView) не показывается.
export const question = $state<{ text: string; ok: string; danger: boolean; resolve: ((v: boolean) => void) | null }>({
  text: '',
  ok: '',
  danger: false,
  resolve: null,
});

export function ask(text: string, ok = 'Да', danger = false): Promise<boolean> {
  question.resolve?.(false);
  return new Promise((resolve) => {
    question.text = text;
    question.ok = ok;
    question.danger = danger;
    question.resolve = resolve;
  });
}

export function answer(v: boolean) {
  const r = question.resolve;
  question.resolve = null;
  r?.(v);
}
