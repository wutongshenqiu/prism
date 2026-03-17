export function confirmAction(message: string) {
  return window.confirm(message);
}

export function navigateTo(url: string) {
  window.location.assign(url);
}
