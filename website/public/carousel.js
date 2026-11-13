const carousel = document.querySelector('.carousel');

if (carousel) {
  const track = carousel.querySelector('.carousel-track');
  const slides = [...carousel.querySelectorAll('.carousel-slide')];
  const dots = [...carousel.querySelectorAll('.carousel-dot')];
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  let active = 0;
  let timer;

  function update(index) {
    active = index;
    dots.forEach((dot, i) => {
      if (i === index) dot.setAttribute('aria-current', 'true');
      else dot.removeAttribute('aria-current');
    });
  }

  function goTo(index) {
    const next = (index + slides.length) % slides.length;
    update(next);
    track.scrollTo({
      left: slides[next].offsetLeft - slides[0].offsetLeft,
      behavior: reducedMotion.matches ? 'instant' : 'smooth',
    });
  }

  function stop() {
    window.clearInterval(timer);
    timer = undefined;
  }

  function start() {
    stop();
    if (!reducedMotion.matches && !document.hidden && !carousel.matches(':hover') && !carousel.contains(document.activeElement)) {
      timer = window.setInterval(() => goTo(active + 1), 5500);
    }
  }

  carousel.querySelector('[data-carousel-prev]').addEventListener('click', () => { goTo(active - 1); start(); });
  carousel.querySelector('[data-carousel-next]').addEventListener('click', () => { goTo(active + 1); start(); });
  dots.forEach((dot, index) => dot.addEventListener('click', () => { goTo(index); start(); }));

  let scrollFrame;
  track.addEventListener('scroll', () => {
    window.cancelAnimationFrame(scrollFrame);
    scrollFrame = window.requestAnimationFrame(() => {
      const index = Math.round(track.scrollLeft / track.clientWidth);
      update(Math.max(0, Math.min(slides.length - 1, index)));
    });
  }, { passive: true });

  carousel.addEventListener('mouseenter', stop);
  carousel.addEventListener('mouseleave', start);
  carousel.addEventListener('focusin', stop);
  carousel.addEventListener('focusout', () => window.setTimeout(start, 0));
  document.addEventListener('visibilitychange', start);
  reducedMotion.addEventListener('change', start);
  start();
}
