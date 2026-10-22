/* opi hero — a quiet three.js scene around the keycap sprite. */
import * as THREE from '../vendor/three.module.min.js';

const canvas = document.getElementById('pet-stage');
const hero = document.getElementById('hero');
if (!canvas || !hero) throw new Error('hero stage missing');

const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const isMobile = window.matchMedia('(max-width: 720px)').matches;

let renderer;
try {
  renderer = new THREE.WebGLRenderer({
    canvas,
    alpha: true,
    antialias: false,
    powerPreference: 'low-power',
  });
} catch (error) {
  // Keep the static pet; the CSS fallback is the whole scene in this case.
  canvas.hidden = true;
  throw error;
}

// Software WebGL (SwiftShader/llvmpipe) can cost seconds per frame; keep the CSS pet instead.
const gl = renderer.getContext();
const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
const rendererName = debugInfo ? String(gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL)) : '';
if (/swiftshader|llvmpipe|software/i.test(rendererName)) {
  renderer.dispose();
  canvas.hidden = true;
  throw new Error('opi: software WebGL, using the static pet');
}

renderer.setClearColor(0x000000, 0);
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.25));
renderer.outputColorSpace = THREE.SRGBColorSpace;

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(34, 1, 0.1, 40);
camera.position.set(0, 0.05, 8.6);

const root = new THREE.Group();
root.position.y = 0.1;
scene.add(root);

function radialTexture(inner, outer, edge = 'rgba(8,12,17,0)') {
  const size = 256;
  const c = document.createElement('canvas');
  c.width = size;
  c.height = size;
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  grad.addColorStop(0, inner);
  grad.addColorStop(0.55, outer);
  grad.addColorStop(1, edge);
  g.fillStyle = grad;
  g.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

const halo = new THREE.Sprite(new THREE.SpriteMaterial({
  map: radialTexture('rgba(232,116,59,.32)', 'rgba(47,143,138,.08)'),
  transparent: true,
  depthWrite: false,
  blending: THREE.AdditiveBlending,
  opacity: 0.75,
}));
halo.scale.set(6.8, 6.8, 1);
root.add(halo);

const ringMaterial = new THREE.MeshBasicMaterial({
  color: 0xe8743b,
  transparent: true,
  opacity: 0.2,
  side: THREE.DoubleSide,
  depthWrite: false,
});
const ringA = new THREE.Mesh(new THREE.RingGeometry(2.62, 2.635, 128), ringMaterial);
ringA.rotation.set(Math.PI / 2.15, 0.28, 0.15);
root.add(ringA);
const ringB = new THREE.Mesh(new THREE.RingGeometry(3.05, 3.07, 128), ringMaterial.clone());
ringB.material.color.setHex(0x2f8f8a);
ringB.material.opacity = 0.14;
ringB.rotation.set(Math.PI / 2.4, -0.44, -0.2);
root.add(ringB);

const glow = new THREE.Sprite(new THREE.SpriteMaterial({
  map: radialTexture('rgba(47,143,138,.3)', 'rgba(47,143,138,.05)'),
  transparent: true,
  depthWrite: false,
  blending: THREE.AdditiveBlending,
  opacity: 0.85,
}));
glow.scale.set(4.5, 4.5, 1);
glow.position.set(0, -0.7, -0.2);
root.add(glow);

const petTexture = new THREE.TextureLoader().load(
  new URL('../assets/pet/opi-pet.png', import.meta.url).href,
  () => {
    document.documentElement.classList.add('three-ready');
    if (reduceMotion) renderer.render(scene, camera);
  },
  undefined,
  () => { /* keep the CSS pet fallback if the texture cannot load */ },
);
petTexture.colorSpace = THREE.SRGBColorSpace;
petTexture.minFilter = THREE.LinearFilter;
petTexture.magFilter = THREE.LinearFilter;
petTexture.generateMipmaps = false;

const pet = new THREE.Mesh(
  new THREE.PlaneGeometry(4.15, 4.15),
  new THREE.MeshBasicMaterial({ map: petTexture, transparent: true, depthWrite: false }),
);
pet.position.z = 0.15;
root.add(pet);

const shadowTexture = radialTexture('rgba(0,0,0,.42)', 'rgba(0,0,0,.1)');
const shadow = new THREE.Sprite(new THREE.SpriteMaterial({
  map: shadowTexture,
  transparent: true,
  depthWrite: false,
  opacity: 0.55,
}));
shadow.scale.set(3.2, 0.62, 1);
shadow.position.set(0, -2.02, -0.4);
root.add(shadow);

const particleCount = isMobile ? 64 : 140;
const positions = new Float32Array(particleCount * 3);
const colors = new Float32Array(particleCount * 3);
const palette = [
  new THREE.Color('#e8743b'),
  new THREE.Color('#63b7b1'),
  new THREE.Color('#f5a6b4'),
  new THREE.Color('#ffd166'),
  new THREE.Color('#a2adb8'),
];
let seed = 20261020;
const random = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
};
for (let i = 0; i < particleCount; i += 1) {
  const radius = 2.55 + random() * 2.2;
  const theta = random() * Math.PI * 2;
  const phi = Math.acos(2 * random() - 1);
  positions[i * 3] = radius * Math.sin(phi) * Math.cos(theta);
  positions[i * 3 + 1] = radius * Math.cos(phi) * 0.78;
  positions[i * 3 + 2] = radius * Math.sin(phi) * Math.sin(theta);
  const color = palette[Math.floor(random() * palette.length)];
  colors[i * 3] = color.r;
  colors[i * 3 + 1] = color.g;
  colors[i * 3 + 2] = color.b;
}
const particleGeometry = new THREE.BufferGeometry();
particleGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
particleGeometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
const particles = new THREE.Points(particleGeometry, new THREE.PointsMaterial({
  size: isMobile ? 0.055 : 0.045,
  vertexColors: true,
  transparent: true,
  opacity: 0.82,
  depthWrite: false,
  blending: THREE.AdditiveBlending,
  sizeAttenuation: true,
}));
root.add(particles);

const target = { x: 0, y: 0 };
const current = { x: 0, y: 0 };
if (!reduceMotion) {
  window.addEventListener('pointermove', (event) => {
    if (!running) return;
    target.x = (event.clientX / window.innerWidth - 0.5) * 2;
    target.y = (event.clientY / window.innerHeight - 0.5) * 2;
  }, { passive: true });
}

function resize() {
  const width = canvas.clientWidth || canvas.parentElement?.clientWidth || 1;
  const height = canvas.clientHeight || canvas.parentElement?.clientHeight || 1;
  renderer.setSize(width, height, false);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  if (reduceMotion) renderer.render(scene, camera);
}

if ('ResizeObserver' in window) {
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas.parentElement || canvas);
} else {
  window.addEventListener('resize', resize, { passive: true });
}
resize();

let heroVisible = true;
const clock = new THREE.Clock();
let running = false;
// Yield the main thread for a few frames around any input. A single 3D frame
// landing between click and paint was the only path that pushed a dialog open
// past the 50 ms response budget.
let interactionUntil = 0;
let interactionTimer = 0;
const yieldToInput = () => {
  interactionUntil = performance.now() + 350;
  syncLoop();
  window.clearTimeout(interactionTimer);
  interactionTimer = window.setTimeout(syncLoop, 380);
};
window.addEventListener('pointerdown', yieldToInput, { passive: true });
window.addEventListener('keydown', yieldToInput, { passive: true });

const canRender = () => !reduceMotion && heroVisible && !document.hidden
  && performance.now() >= interactionUntil
  && !document.body.classList.contains('memory-open');

const syncLoop = () => {
  const next = canRender();
  if (next === running) return;
  running = next;
  if (running) {
    clock.getDelta();
    frame();
  }
};

let lastFrameAt = 0;
function frame() {
  if (!running) return;
  window.requestAnimationFrame(frame);
  if (!canRender()) {
    running = false;
    return;
  }
  // Decorative scene: 30 fps leaves the main thread room for input/paint.
  const now = performance.now();
  if (now - lastFrameAt < 32) return;
  lastFrameAt = now;
  const t = clock.getElapsedTime();
  current.x += (target.x - current.x) * 0.045;
  current.y += (target.y - current.y) * 0.045;

  root.rotation.y = Math.sin(t * 0.24) * 0.16 + current.x * 0.22;
  root.rotation.x = -0.025 + Math.sin(t * 0.38) * 0.025 + current.y * 0.07;
  pet.position.y = Math.sin(t * 0.9) * 0.075;
  pet.rotation.z = Math.sin(t * 0.62) * 0.012 + current.x * 0.015;
  glow.material.opacity = 0.78 + Math.sin(t * 1.2) * 0.08;
  particles.rotation.y = t * 0.032;
  particles.rotation.x = Math.sin(t * 0.12) * 0.06;
  ringA.rotation.z = 0.15 + t * 0.02;
  ringB.rotation.z = -0.2 - t * 0.016;

  renderer.render(scene, camera);
}

if ('IntersectionObserver' in window) {
  new IntersectionObserver((entries) => {
    heroVisible = entries.some((entry) => entry.isIntersecting);
    syncLoop();
  }, { threshold: 0 }).observe(hero);
}
document.addEventListener('visibilitychange', syncLoop);
new MutationObserver(syncLoop).observe(document.body, { attributes: true, attributeFilter: ['class'] });


if (reduceMotion) renderer.render(scene, camera);
else syncLoop();
