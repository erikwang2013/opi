/* opi static site — navigation, reveal, pet-form dialog. No framework. */
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
  const toTop = $('.back-to-top');
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
    if (toTop) toTop.classList.toggle('is-visible', y > window.innerHeight * 0.8);

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

  toTop?.addEventListener('click', () => {
    window.scrollTo({ top: 0, behavior: reduceMotion ? 'auto' : 'smooth' });
  });

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

  // History memory timeline: first visit auto-plays once; the "回忆" button replays it.
  const memoryOverlay = $('#memory-overlay');
  const memoryViewport = $('#memory-viewport');
  const memoryTimeline = $('#memory-timeline');
  const memoryProgress = $('#memory-progress');
  const memoryIntro = $('#memory-intro');
  const memoryIntroImg = $('#memory-intro-img');
  const memoryIntroLabel = $('#memory-intro-label');
  const memoryIntroCount = $('#memory-intro-count');
  const memoryClose = memoryOverlay ? $('[data-memory-close]', memoryOverlay) : null;
  const memorySeenKey = 'opi-memory-seen-v1';
  const memoryPets = [
    'idle.gif', 'waiting.gif', 'crying.gif', 'puzzled.gif', 'terrified.gif',
    'angry.gif', 'coquettish.gif', 'cute.gif', 'pleasing.gif', 'knocking.gif',
    'face-palm.gif', 'tongue-out.gif',
  ];
  const memoryIntroForms = [
    ['idle.gif', '小欧·笑'], ['waiting.gif', '小欧·欢乐'], ['crying.gif', '小欧·哭'],
    ['puzzled.gif', '小欧·沮丧'], ['terrified.gif', '小欧·惊恐'], ['angry.gif', '小欧·愤怒'],
    ['coquettish.gif', '小欧·撒娇'], ['cute.gif', '小欧·卖萌'], ['pleasing.gif', '小欧·讨好'],
    ['knocking.gif', '小欧·敲打'], ['face-palm.gif', '小欧·捂脸'], ['tongue-out.gif', '小欧·吐舌头'],
  ];
  // Warm the local GIF/PNG cache so the opening sequence never shows an empty box.
  memoryIntroForms.forEach(([file]) => { const img = new Image(); img.src = `assets/pet/forms/${file}`; });
  let memoryBuilt = false;
  let memoryRaf = 0;
  let memoryEndTimer = 0;
  let memoryIntroTimer = 0;
  let memoryObserver = null;
  let memoryRevealTimer = 0;
  let memoryRevealQueue = [];
  let memoryPreviousFocus = null;
  let memoryPaused = false;

  const markMemorySeen = () => {
    try { localStorage.setItem(memorySeenKey, '1'); } catch (_) { /* private mode */ }
  };

  const buildMemory = () => {
    if (memoryBuilt || !memoryTimeline) return;
    memoryBuilt = true;
    const source = $$('.release-latest, .release-item a', $('.release-board') || document);
    const rows = source.map((link) => {
      const item = link.classList.contains('release-latest') ? link : link;
      return {
        version: $('.release-version', item)?.textContent?.trim() || '',
        date: $('.release-date', item)?.textContent?.trim() || '',
        note: $('.release-note', item)?.textContent?.trim() || '',
      };
    }).filter((row) => row.version).reverse(); // oldest → newest
    const nodes = [];
    rows.forEach((row, index) => {
      const li = document.createElement('li');
      li.className = 'memory-item' + (index === rows.length - 1 ? ' is-latest' : '');
      const version = document.createElement('span');
      version.className = 'memory-version';
      version.textContent = row.version;
      const date = document.createElement('span');
      date.className = 'memory-date';
      date.textContent = row.date;
      const note = document.createElement('span');
      note.className = 'memory-note';
      note.textContent = row.note;
      li.append(version, date, note);
      nodes.push(li);
      if (index % 6 === 2) {
        const pet = document.createElement('li');
        pet.className = 'memory-pet' + (index % 12 === 9 ? ' is-right' : '');
        pet.setAttribute('aria-hidden', 'true');
        const img = document.createElement('img');
        img.src = `assets/pet/forms/${memoryPets[(index / 6 | 0) % memoryPets.length]}`;
        img.width = 54;
        img.height = 54;
        img.loading = 'lazy';
        img.decoding = 'async';
        img.alt = '';
        pet.append(img);
        nodes.push(pet);
      }
    });
    memoryTimeline.replaceChildren(...nodes);
  };

  const setMemoryPaused = (paused) => { memoryPaused = paused; };

  const memoryProgressNow = () => {
    if (!memoryViewport || !memoryProgress) return;
    const max = Math.max(1, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    memoryProgress.style.transform = `scaleX(${Math.min(1, memoryViewport.scrollTop / max)})`;
  };

  const stopMemory = () => {
    if (memoryRaf) cancelAnimationFrame(memoryRaf);
    if (memoryEndTimer) clearTimeout(memoryEndTimer);
    if (memoryIntroTimer) clearTimeout(memoryIntroTimer);
    if (memoryRevealTimer) clearTimeout(memoryRevealTimer);
    memoryObserver?.disconnect();
    memoryObserver = null;
    memoryRevealQueue = [];
    memoryRaf = 0;
    memoryEndTimer = 0;
    memoryIntroTimer = 0;
    memoryRevealTimer = 0;
    memoryIntroImg?.getAnimations().forEach((animation) => animation.cancel());
    memoryIntro?.classList.remove('is-showing', 'is-pop');
  };

  const closeMemory = () => {
    if (!memoryOverlay || !memoryOverlay.classList.contains('is-open')) return;
    stopMemory();
    memoryOverlay.classList.remove('is-open');
    document.body.classList.remove('memory-open');
    memoryPreviousFocus?.focus?.({ preventScroll: true });
    window.setTimeout(() => {
      if (memoryOverlay && !memoryOverlay.classList.contains('is-open')) memoryOverlay.hidden = true;
    }, 240);
  };

  const resetMemoryItems = () => {
    if (!memoryTimeline) return;
    $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => {
      el.classList.remove('is-in', 'is-current');
      delete el.dataset.memoryQueued;
    });
  };

  const revealMemoryNext = () => {
    memoryRevealTimer = 0;
    const next = memoryRevealQueue.shift();
    if (next) {
      if (next.classList.contains('memory-item')) {
        const current = memoryTimeline?.querySelector('.memory-item.is-current');
        if (current && current !== next) current.classList.remove('is-current');
        next.classList.add('is-in', 'is-current');
      } else {
        next.classList.add('is-in');
      }
      delete next.dataset.memoryQueued;
    }
    if (memoryRevealQueue.length) memoryRevealTimer = window.setTimeout(revealMemoryNext, 180);
  };

  const queueMemoryReveal = (el) => {
    if (!el || el.classList.contains('is-in') || el.dataset.memoryQueued) return;
    el.dataset.memoryQueued = '1';
    memoryRevealQueue.push(el);
    if (!memoryRevealTimer && memoryRevealQueue.length === 1) revealMemoryNext();
  };

  const startMemoryIntro = () => {
    if (!memoryIntro || !memoryIntroImg || !memoryIntroLabel || !memoryIntroCount || reduceMotion) {
      playMemory();
      return;
    }
    stopMemory();
    let index = 0;
    memoryIntro.classList.add('is-showing');
    const show = () => {
      const [file, label] = memoryIntroForms[index];
      memoryIntroImg.src = `assets/pet/forms/${file}`;
      memoryIntroLabel.textContent = label;
      memoryIntroCount.textContent = `${index + 1} / ${memoryIntroForms.length}`;
      const entry = index % 3;
      const from = entry === 0
        ? 'translate3d(-72vw, 18vh, 0) scale(.12) rotate(-150deg)'
        : entry === 1
          ? 'translate3d(70vw, 34vh, 0) scale(.16) rotate(140deg)'
          : 'translate3d(0, 68vh, 0) scale(.1) rotate(80deg)';
      memoryIntroImg.animate(
        [{ transform: from, opacity: 0 },
         { opacity: 1, offset: .35 },
         { transform: 'translate3d(0, 0, 0) scale(1.08) rotate(-3deg)', offset: .8 },
         { transform: 'none' }],
        { duration: 880, easing: 'cubic-bezier(.16,1,.3,1)' },
      );
      index += 1;
      if (index < memoryIntroForms.length) {
        memoryIntroTimer = window.setTimeout(show, 980);
      } else {
        memoryIntroTimer = window.setTimeout(() => {
          memoryIntro.classList.remove('is-showing');
          memoryIntroTimer = window.setTimeout(playMemory, 280);
        }, 760);
      }
    };
    show();
  };

  const playMemory = () => {
    if (!memoryTimeline || !memoryViewport || !memoryProgress) return;
    stopMemory();
    resetMemoryItems();
    memoryViewport.scrollTop = 0;
    memoryProgressNow();
    if (reduceMotion) {
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => el.classList.add('is-in'));
      return;
    }
    if ('IntersectionObserver' in window) {
      memoryObserver = new IntersectionObserver((entries) => {
        entries.forEach((entry) => { if (entry.isIntersecting) queueMemoryReveal(entry.target); });
      }, { root: memoryViewport, rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => memoryObserver.observe(el));
    } else {
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => el.classList.add('is-in'));
    }
    const max = Math.max(0, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    if (max <= 0) return;
    // ~2 minutes for 76 versions; the wheel scrolls natively and pauses auto-advance.
    const speed = max / 120000; // px per ms
    let last = 0;
    const step = (now) => {
      if (!last) last = now;
      const dt = Math.min(80, Math.max(0, now - last));
      last = now;
      if (!memoryPaused) {
        memoryViewport.scrollTop = Math.min(max, memoryViewport.scrollTop + speed * dt);
        if (memoryViewport.scrollTop >= max - 1) {
          memoryProgressNow();
          memoryEndTimer = window.setTimeout(closeMemory, 1200);
          return;
        }
      }
      memoryProgressNow();
      memoryRaf = requestAnimationFrame(step);
    };
    memoryRaf = requestAnimationFrame(step);
  };

  const openMemory = () => {
    if (!memoryOverlay) return;
    buildMemory();
    if (memoryOverlay.classList.contains('is-open')) return;
    memoryPreviousFocus = document.activeElement;
    memoryPaused = false;
    memoryViewport.scrollTop = 0;
    memoryOverlay.hidden = false;
    document.body.classList.add('memory-open');
    markMemorySeen();
    memoryClose?.focus?.({ preventScroll: true });
    window.requestAnimationFrame(() => {
      memoryOverlay.classList.add('is-open');
      startMemoryIntro();
    });
  };

  if (memoryOverlay) {
    $$('[data-memory-open]').forEach((button) => button.addEventListener('click', openMemory));
    memoryClose?.addEventListener('click', closeMemory);
    memoryOverlay.addEventListener('click', (event) => {
      if (event.target === memoryOverlay) closeMemory();
    });
    memoryViewport?.addEventListener('pointerenter', () => setMemoryPaused(true));
    memoryViewport?.addEventListener('pointerleave', () => setMemoryPaused(false));
    memoryViewport?.addEventListener('scroll', memoryProgressNow, { passive: true });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') closeMemory();
    });
    let seen = false;
    try { seen = localStorage.getItem(memorySeenKey) === '1'; } catch (_) { /* private mode */ }
    if (!seen) window.setTimeout(openMemory, 1200);
  }

  // Keep the active dot honest when arriving with a hash (e.g. /#download).
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target && target.id) setActive(target.id);
  }
})();
