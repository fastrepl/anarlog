export function observeReferralView(
  element: Element,
  onView: () => void,
  createObserver = (callback: IntersectionObserverCallback) =>
    new IntersectionObserver(callback),
) {
  let viewed = false;
  const observer = createObserver((entries) => {
    if (viewed || !entries.some((entry) => entry.isIntersecting)) return;
    viewed = true;
    observer.disconnect();
    onView();
  });
  observer.observe(element);
  return () => observer.disconnect();
}
