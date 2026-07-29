#!/usr/bin/env python3
import sys
import bisect
import statistics
import subprocess
from datetime import timedelta
from difflib import SequenceMatcher

try:
    import srt
    from faster_whisper import WhisperModel
except ImportError as e:
    print(f"[pw-whisper] missing dep: {e}", file=sys.stderr)
    sys.exit(127)

if len(sys.argv) != 4:
    print("usage: pw-whisper-sync <video> <in.srt> <out.srt>", file=sys.stderr)
    sys.exit(2)

video = sys.argv[1]
in_path = sys.argv[2]
out_path = sys.argv[3]

with open(in_path, encoding="utf-8") as f:
    subs = list(srt.parse(f.read()))

if not subs:
    print("[pw-whisper] empty subtitle", file=sys.stderr)
    sys.exit(3)


def video_duration(path):
    try:
        out = subprocess.check_output([
            "ffprobe", "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path,
        ], timeout=15).decode().strip()
        return float(out)
    except Exception as e:
        print(f"[pw-whisper] ffprobe failed ({e}), using subs span", file=sys.stderr)
        return subs[-1].end.total_seconds()


total = video_duration(video)
last_sub = subs[-1].end.total_seconds()
total = min(total, last_sub + 60)

WINDOW = 90
anchor_centers = [
    min(60, total / 6),
    total * 0.5,
    total * 0.78,
]

print(f"[pw-whisper] loading tiny model (int8) ...")
model = WhisperModel("tiny", compute_type="int8")


def transcribe_window(start, end):
    start = max(0, start)
    end = min(total, end)
    if end <= start:
        return []
    segs, _ = model.transcribe(
        video,
        vad_filter=True,
        beam_size=1,
        clip_timestamps=f"{int(start)},{int(end)}",
    )
    out = []
    for s in segs:
        t = s.text.strip().lower()
        if len(t) >= 3:
            out.append((s.start, t))
    return out


def best_match(sub_text, whisper_segs, sub_t):
    best_score = 0.0
    best_t = None
    for ws, wt in whisper_segs:
        if abs(ws - sub_t) > 30:
            continue
        score = SequenceMatcher(None, sub_text[:60], wt[:60]).ratio()
        if score > best_score:
            best_score = score
            best_t = ws
    return best_score, best_t


def shift_for_window(center):
    win_start = center - WINDOW / 2
    win_end = center + WINDOW / 2
    segs = transcribe_window(win_start, win_end)
    if not segs:
        return None, 0
    offsets = []
    for sub in subs:
        st = sub.start.total_seconds()
        if not (win_start - 20 <= st <= win_end + 20):
            continue
        txt = sub.content.replace("\n", " ").strip().lower()
        if len(txt) < 3:
            continue
        score, t = best_match(txt, segs, st)
        if score >= 0.45 and t is not None:
            offsets.append(t - st)
    if len(offsets) < 2:
        return None, len(offsets)
    return statistics.median(offsets), len(offsets)


anchors = []
for c in anchor_centers:
    shift, n = shift_for_window(c)
    if shift is None:
        print(f"[pw-whisper] anchor @ {c:.0f}s skipped (matches={n})", file=sys.stderr)
        continue
    anchors.append((c, shift))
    print(f"[pw-whisper] anchor @ {c:.0f}s: shift={shift:+.2f}s (matches={n})")

if not anchors:
    print("[pw-whisper] no usable anchors", file=sys.stderr)
    sys.exit(5)

anchors.sort()


def shift_at(t):
    if len(anchors) == 1:
        return anchors[0][1]
    if t <= anchors[0][0]:
        return anchors[0][1]
    if t >= anchors[-1][0]:
        return anchors[-1][1]
    ts = [a[0] for a in anchors]
    i = bisect.bisect_left(ts, t)
    if i < len(ts) and ts[i] == t:
        return anchors[i][1]
    t0, s0 = anchors[i - 1]
    t1, s1 = anchors[i]
    frac = (t - t0) / (t1 - t0)
    return s0 + frac * (s1 - s0)


for sub in subs:
    st = sub.start.total_seconds()
    delta = timedelta(seconds=shift_at(st))
    sub.start += delta
    sub.end += delta

with open(out_path, "w", encoding="utf-8") as f:
    f.write(srt.compose(subs))

print(f"[pw-whisper] wrote {out_path} (anchors={len(anchors)})")
