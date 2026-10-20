/* opi static site — memory timeline overlay. Loaded with defer after site.js. */
(() => {
  'use strict';
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const memoryOverlay = $('#memory-overlay');
  const memoryViewport = $('#memory-viewport');
  const memoryTimeline = $('#memory-timeline');
  const memoryProgress = $('#memory-progress');
  const memoryIntro = $('#memory-intro');
  const memoryClose = memoryOverlay ? $('[data-memory-close]', memoryOverlay) : null;
  const memorySeenKey = 'opi-memory-seen-v1';
  const memoryPets = [
    'idle.gif', 'waiting.gif', 'crying.gif', 'puzzled.gif', 'terrified.gif',
    'angry.gif', 'coquettish.gif', 'cute.gif', 'pleasing.gif', 'knocking.gif',
    'face-palm.gif', 'tongue-out.gif',
  ];
  const memoryIntroForms = [
    ['idle.gif', '小欧·笑', '#F5A6B4'], ['waiting.gif', '小欧·欢乐', '#F27D9D'],
    ['crying.gif', '小欧·哭', '#4E97E6'], ['puzzled.gif', '小欧·沮丧', '#7E8C86'],
    ['terrified.gif', '小欧·惊恐', '#6FB2F0'], ['angry.gif', '小欧·愤怒', '#DD5138'],
    ['coquettish.gif', '小欧·撒娇', '#F46F9B'], ['cute.gif', '小欧·卖萌', '#F2B84B'],
    ['pleasing.gif', '小欧·讨好', '#5FBF9F'], ['knocking.gif', '小欧·敲打', '#E8A13C'],
    ['face-palm.gif', '小欧·捂脸', '#E06A4E'], ['tongue-out.gif', '小欧·吐舌头', '#EF5D8F'],
  ];
  const preloadMemoryPets = () => {
    memoryIntroForms.forEach(([file]) => { const img = new Image(); img.src = `assets/pet/forms/${file}`; });
  };

  let memoryBuilt = false;
  let memoryIntroSingle = null;
  let memoryIntroImg = null;
  let memoryIntroLabel = null;
  let memoryIntroCount = null;
  let memoryFormsEntry = null;
  let memoryRaf = 0;
  let memoryEndTimer = 0;
  let memoryIntroTimer = 0;
  let memoryLoadQueue = [];
  let memoryNextStepAt = 0;
  let memoryScroll = null;
  let memoryPausedAt = 0;
  let memoryPreviousFocus = null;
  let memoryPaused = false;

  const markMemorySeen = () => {
    try { localStorage.setItem(memorySeenKey, '1'); } catch (_) { /* private mode */ }
  };

  const buildFormsEntry = () => {
    const entry = document.createElement('li');
    entry.className = 'memory-forms-entry';
    const title = document.createElement('p');
    title.className = 'memory-forms-title';
    title.textContent = '小欧的十二种形态 · 故事从这里开始';
    const grid = document.createElement('div');
    grid.className = 'memory-forms-grid';
    memoryIntroForms.forEach(([file, name, mood]) => {
      const item = document.createElement('figure');
      item.className = 'memory-forms-pet';
      item.style.setProperty('--mood', mood);
      const img = document.createElement('img');
      img.src = `assets/pet/forms/${file}`;
      img.width = 48;
      img.height = 48;
      img.alt = name;
      img.loading = 'eager';
      img.decoding = 'async';
      const caption = document.createElement('figcaption');
      caption.textContent = name;
      item.append(img, caption);
      grid.append(item);
    });
    entry.append(title, grid);
    return entry;
  };

  const buildMemory = () => {
    if (memoryBuilt || !memoryTimeline) return;
    memoryBuilt = true;
    const source = $$('.release-latest, .release-item a', $('.release-board') || document);
    const rows = source.map((item) => ({
      version: $('.release-version', item)?.textContent?.trim() || '',
      date: $('.release-date', item)?.textContent?.trim() || '',
      note: $('.release-note', item)?.textContent?.trim() || '',
    })).filter((row) => row.version); // newest → oldest (top → bottom), loading upward from v1.0.0
    const nodes = [];
    rows.forEach((row, index) => {
      const li = document.createElement('li');
      li.className = 'memory-item' + (index === 0 ? ' is-latest' : '');
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
    memoryFormsEntry = buildFormsEntry();
    nodes.push(memoryFormsEntry);
    memoryTimeline.replaceChildren(...nodes);
  };

  const buildMemoryIntro = () => {
    if (!memoryIntro || memoryIntro.children.length) return;
    const single = document.createElement('div');
    single.className = 'memory-intro-pane memory-intro-single';
    const img = document.createElement('img');
    img.width = 112;
    img.height = 112;
    img.alt = '';
    const label = document.createElement('p');
    const count = document.createElement('span');
    single.append(img, label, count);
    memoryIntro.append(single);
    memoryIntroSingle = single;
    memoryIntroImg = img;
    memoryIntroLabel = label;
    memoryIntroCount = count;
  };

  const setMemoryPaused = (paused) => {
    if (paused === memoryPaused) return;
    const now = performance.now();
    if (paused) {
      memoryPausedAt = now;
    } else if (memoryPausedAt) {
      const delta = now - memoryPausedAt;
      memoryNextStepAt += delta;
      if (memoryScroll) {
        memoryScroll.from = memoryViewport.scrollTop;
        memoryScroll.start = now;
      }
      memoryPausedAt = 0;
    }
    memoryPaused = paused;
  };

  // 手动滚轮/触摸时放弃当前自动滚动段，避免和用户抢滚动位置。
  const interruptMemoryScroll = () => {
    if (!memoryScroll) return;
    memoryScroll = null;
    memoryNextStepAt = Math.max(memoryNextStepAt, performance.now() + 400);
  };

  const memoryProgressNow = () => {
    if (!memoryViewport || !memoryProgress) return;
    const max = Math.max(1, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    memoryProgress.style.transform = `scaleX(${Math.min(1, 1 - memoryViewport.scrollTop / max)})`;
  };

  // 先把当前窗口里已经出现的条目加载出来，之后滚动始终跟着已加载内容走。
  const primeMemoryWindow = () => {
    if (!memoryViewport) return;
    const view = memoryViewport.getBoundingClientRect();
    const visible = $$('.memory-item, .memory-pet', memoryTimeline).filter((el) => {
      const rect = el.getBoundingClientRect();
      return rect.bottom > view.top && rect.top < view.bottom;
    });
    visible.forEach((el) => el.classList.add('is-in'));
    visible.filter((el) => el.classList.contains('memory-item')).pop()?.classList.add('is-current');
  };

  const stopMemory = () => {
    if (memoryRaf) cancelAnimationFrame(memoryRaf);
    if (memoryEndTimer) clearTimeout(memoryEndTimer);
    if (memoryIntroTimer) clearTimeout(memoryIntroTimer);
    memoryRaf = 0;
    memoryEndTimer = 0;
    memoryIntroTimer = 0;
    memoryLoadQueue = [];
    memoryScroll = null;
    memoryNextStepAt = 0;
    memoryPausedAt = 0;
    memoryIntro?.querySelectorAll('img').forEach((img) => img.getAnimations().forEach((a) => a.cancel()));
    memoryIntro?.classList.remove('is-showing');
    memoryIntroSingle?.classList.remove('is-active');
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
    $$('.memory-item, .memory-pet, .memory-forms-entry', memoryTimeline).forEach((el) => {
      el.classList.remove('is-in', 'is-current');
      delete el.dataset.memoryQueued;
    });
  };

  const introEntry = (index) => {
    const entry = index % 3;
    if (entry === 0) return 'translate3d(-70vw, 18vh, 0) scale(.18) rotate(-150deg)';
    if (entry === 1) return 'translate3d(70vw, 30vh, 0) scale(.2) rotate(140deg)';
    return 'translate3d(0, 62vh, 0) scale(.16) rotate(80deg)';
  };

  const startMemoryIntro = () => {
    if (!memoryIntro || reduceMotion) { playMemory(); return; }
    buildMemoryIntro();
    stopMemory();
    memoryIntro.classList.add('is-showing');
    memoryIntroSingle.classList.add('is-active');
    let index = 0;
    const show = () => {
      const [file, label, mood] = memoryIntroForms[index];
      memoryIntro.style.setProperty('--intro-mood', mood);
      memoryIntroImg.src = `assets/pet/forms/${file}`;
      memoryIntroImg.alt = label;
      memoryIntroLabel.textContent = label;
      memoryIntroCount.textContent = `${index + 1} / ${memoryIntroForms.length}`;
      memoryIntroImg.animate(
        [{ transform: introEntry(index) },
         { transform: 'scale(1.06) rotate(-3deg)', offset: .8 },
         { transform: 'none' }],
        { duration: 720, easing: 'cubic-bezier(.16,1,.3,1)' },
      );
      index += 1;
      if (index < memoryIntroForms.length) {
        memoryIntroTimer = window.setTimeout(show, 1800);
      } else {
        memoryIntroTimer = window.setTimeout(() => {
          memoryIntro.classList.remove('is-showing');
          memoryIntroSingle.classList.remove('is-active');
          memoryIntroTimer = window.setTimeout(playMemory, 300);
        }, 900);
      }
    };
    show();
  };

  const playMemory = () => {
    if (!memoryTimeline || !memoryViewport || !memoryProgress) return;
    stopMemory();
    resetMemoryItems();
    memoryViewport.scrollTop = memoryViewport.scrollHeight;
    if (memoryFormsEntry) memoryFormsEntry.classList.add('is-in');
    primeMemoryWindow();
    memoryProgressNow();
    if (reduceMotion) {
      $$('.memory-item, .memory-pet, .memory-forms-entry', memoryTimeline).forEach((el) => el.classList.add('is-in'));
      return;
    }
    memoryLoadQueue = $$('.memory-item, .memory-pet', memoryTimeline)
      .filter((el) => !el.classList.contains('is-in'))
      .reverse(); // bottom-up: load one history entry, then scroll to it
    if (!memoryLoadQueue.length) return;
    const stepMs = 1900;
    memoryNextStepAt = performance.now() + 3000;
    const step = (now) => {
      if (memoryPaused) {
        memoryRaf = requestAnimationFrame(step);
        return;
      }
      if (!memoryScroll && memoryLoadQueue.length && now >= memoryNextStepAt) {
        const next = memoryLoadQueue.shift();
        if (next.classList.contains('memory-item')) {
          memoryTimeline.querySelector('.memory-item.is-current')?.classList.remove('is-current');
          next.classList.add('is-in', 'is-current');
        } else {
          next.classList.add('is-in');
        }
        const from = memoryViewport.scrollTop;
        const target = Math.min(from, Math.max(0, next.offsetTop - 6));
        memoryScroll = { from, to: target, start: now, duration: stepMs };
        memoryNextStepAt = now + stepMs;
      }
      if (memoryScroll) {
        const progress = Math.min(1, (now - memoryScroll.start) / memoryScroll.duration);
        memoryViewport.scrollTop = memoryScroll.from + (memoryScroll.to - memoryScroll.from) * progress;
        if (progress >= 1) memoryScroll = null;
      }
      memoryProgressNow();
      if (!memoryLoadQueue.length && !memoryScroll) {
        memoryEndTimer = window.setTimeout(closeMemory, 1600);
        return;
      }
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
    memoryPausedAt = 0;
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
    if ('requestIdleCallback' in window) window.requestIdleCallback(preloadMemoryPets, { timeout: 1800 });
    else window.setTimeout(preloadMemoryPets, 600);
    $$('[data-memory-open]').forEach((button) => button.addEventListener('click', openMemory));
    memoryClose?.addEventListener('click', closeMemory);
    memoryOverlay.addEventListener('click', (event) => { if (event.target === memoryOverlay) closeMemory(); });
    memoryViewport?.addEventListener('pointerenter', () => setMemoryPaused(true));
    memoryViewport?.addEventListener('pointerleave', () => setMemoryPaused(false));
    memoryViewport?.addEventListener('scroll', memoryProgressNow, { passive: true });
    memoryViewport?.addEventListener('wheel', interruptMemoryScroll, { passive: true });
    memoryViewport?.addEventListener('touchstart', interruptMemoryScroll, { passive: true });
    document.addEventListener('keydown', (event) => { if (event.key === 'Escape') closeMemory(); });
    let seen = false;
    try { seen = localStorage.getItem(memorySeenKey) === '1'; } catch (_) { /* private mode */ }
    if (!seen) window.setTimeout(openMemory, 1200);
  }
})();
