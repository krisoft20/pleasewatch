const seen = new IntersectionObserver(
  es => es.forEach(e => { if (e.isIntersecting) { e.target.classList.add('in'); seen.unobserve(e.target); } }),
  { threshold: 0.6 }
);
document.querySelectorAll('.draw, .swipe').forEach(el => seen.observe(el));

document.querySelectorAll('[data-copy]').forEach(b => {
  const label = b.textContent;
  b.addEventListener('click', () => {
    const text = b.parentElement.querySelector('code').textContent.trim();
    navigator.clipboard.writeText(text).then(() => {
      b.textContent = 'copied!';
      setTimeout(() => (b.textContent = label), 1500);
    });
  });
});

const words = [...document.querySelectorAll('.strip button')];
const light = k => words.forEach(w => w.classList.toggle('on', w.dataset.k === k));
words.forEach(w => {
  w.addEventListener('pointerenter', () => light(w.dataset.k));
  w.addEventListener('focus', () => light(w.dataset.k));
  w.addEventListener('pointerleave', () => light(''));
  w.addEventListener('blur', () => light(''));
  w.addEventListener('click', () => {
    light(w.dataset.k);
    if (!('speechSynthesis' in window)) return;
    const u = new SpeechSynthesisUtterance(w.textContent.replace(/[.,]/g, ''));
    u.lang = w.parentElement.lang === 'pl' ? 'pl-PL' : 'en-US';
    speechSynthesis.cancel();
    speechSynthesis.speak(u);
  });
});

const room = document.querySelector('.room');
if (room) {
  const [a, b] = room.querySelectorAll('output');
  const [sa, sb] = room.querySelectorAll('.screen i');
  const wait = room.querySelector('input');
  const face = room.querySelector('.ctl img');
  let clock = 754, lag = 0, stall = 0, last = performance.now();
  const fmt = s => `${String(Math.floor(s / 60)).padStart(2, '0')}:${(s % 60).toFixed(1).padStart(4, '0')}`;

  function tick(now) {
    const dt = (now - last) / 1000;
    last = now;
    if (stall > 0) {
      stall -= dt;
      if (wait.checked) {
        sa.textContent = 'waiting for kasia';
      } else {
        clock += dt;
        lag += dt;
      }
      sb.textContent = 'buffering...';
      if (stall <= 0) face.src = 'img/tv-wink.svg';
    } else {
      clock += dt;
      if (lag > 0) {
        lag = Math.max(0, lag - dt * 0.6);
        sb.textContent = lag > 0 ? 'catching up' : '';
      } else {
        sb.textContent = '';
      }
      sa.textContent = '';
    }
    a.textContent = fmt(clock);
    b.textContent = fmt(clock - lag);
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);

  room.querySelector('button').addEventListener('click', () => {
    stall = 2.5;
    face.src = 'img/tv-oh.svg';
  });
}
