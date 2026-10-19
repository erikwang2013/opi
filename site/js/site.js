/* OPI static site — navigation, reveal, pet-form dialog. No framework. */
(() => {
  'use strict';

  const doc = document.documentElement;
  doc.classList.add('js');

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const year = $('#year');
  if (year) year.textContent = String(new Date().getFullYear());

  const header = $('.site-header');
  const progress = $('.scroll-progress span');
  const sections = $$('main > section[id]');
  const navLinks = $$('.site-nav a[href^="#"]');
  const dots = $$('.section-dots a');
  let offsets = [];
  let activeId = '';

  const measure = () => {
    offsets = sections.map((section) => ({
      id: section.id,
      top: section.offsetTop,
      height: section.offsetHeight,
    }));
  };

  const setActive = (id) => {
    if (!id || id === activeId) return;
    activeId = id;
    navLinks.forEach((link) => {
      if (link.hash === `#${id}`) link.setAttribute('aria-current', 'page');
      else link.removeAttribute('aria-current');
    });
    dots.forEach((dot) => {
      if (dot.dataset.for === id) dot.setAttribute('aria-current', 'true');
      else dot.removeAttribute('aria-current');
    });
  };

  let scheduled = false;
  const update = () => {
    scheduled = false;
    const y = window.scrollY;
    const max = Math.max(1, doc.scrollHeight - window.innerHeight);
    if (progress) progress.style.transform = `scaleX(${Math.min(1, y / max)})`;
    if (header) header.classList.toggle('is-scrolled', y > 12);

    if (!offsets.length) measure();
    const probe = y + window.innerHeight * 0.38;
    let current = offsets[0] && offsets[0].id;
    for (const item of offsets) {
      if (item.top <= probe) current = item.id;
    }
    setActive(current);
  };

  const requestUpdate = () => {
    if (scheduled) return;
    scheduled = true;
    window.requestAnimationFrame(update);
  };

  window.addEventListener('scroll', requestUpdate, { passive: true });
  window.addEventListener('resize', () => { measure(); requestUpdate(); }, { passive: true });
  window.addEventListener('load', () => { measure(); requestUpdate(); }, { once: true });
  measure();
  update();

  // Reveal on entry — one observer for all sections and cards.
  const revealTargets = $$('.reveal');
  if ('IntersectionObserver' in window && !reduceMotion) {
    const revealObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add('is-visible');
        revealObserver.unobserve(entry.target);
      });
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
    revealTargets.forEach((el) => revealObserver.observe(el));
  } else {
    revealTargets.forEach((el) => el.classList.add('is-visible'));
  }

  // Reduced motion: use the static PNG in place of the GIF.
  if (reduceMotion) {
    $$('img[data-still]').forEach((img) => { img.src = img.dataset.still; });
  }

  // Pet-form dialog.
  const dialog = $('#form-dialog');
  const dialogImage = $('#dialog-image');
  const dialogName = $('#dialog-name');
  const dialogNote = $('#dialog-note');

  const closeDialog = () => {
    if (dialog && dialog.open) dialog.close();
  };

  if (dialog && typeof dialog.showModal === 'function') {
    $$('.form-card').forEach((card) => {
      card.addEventListener('click', () => {
        const gif = card.dataset.gif;
        const still = card.querySelector('img')?.dataset.still;
        if (dialogImage) dialogImage.src = reduceMotion && still ? still : (gif || still || '');
        if (dialogName) dialogName.textContent = card.dataset.name || '';
        if (dialogNote) dialogNote.textContent = card.dataset.note || '';
        dialog.showModal();
      });
    });

    $('.dialog-close', dialog)?.addEventListener('click', closeDialog);
    dialog.addEventListener('click', (event) => {
      if (event.target === dialog) closeDialog();
    });
  }

  // Keep the active dot honest when arriving with a hash (e.g. /#download).
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target && target.id) setActive(target.id);
  }
})();
