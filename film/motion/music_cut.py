"""Cut a music track to the film's length on its own beat grid.

Takes whole bars out at one point (so the bars on both sides of the join line up), joins
them with a short equal-power crossfade, fades out the last seconds, and sets the level to
the bed's full level (-20 LUFS; the film dips it under the narration).

    C:/Users/vasta/tools/qwen-tts/.venv/Scripts/python.exe -I film/motion/music_cut.py \
        "<track.mp3>" film/audio/music/bed.wav --cut-near 300 --bars 20 --length 360
"""

import argparse

import librosa
import numpy as np
import pyloudnorm as pyln
import soundfile as sf


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("src")
    parser.add_argument("out")
    parser.add_argument("--cut-near", type=float, required=True, help="s in the track where the bars come out")
    parser.add_argument("--bars", type=int, required=True)
    parser.add_argument("--length", type=float, default=360.0, help="s of the finished bed")
    parser.add_argument("--fade-out", type=float, default=4.0)
    parser.add_argument("--lufs", type=float, default=-20.0)
    args = parser.parse_args()

    audio, rate = sf.read(args.src, dtype="float32", always_2d=True)
    mono, beat_rate = librosa.load(args.src, sr=22050, mono=True)
    _, beats = librosa.beat.beat_track(y=mono, sr=beat_rate, units="time")
    i = int(np.argmin(np.abs(beats - args.cut_near)))
    a, b = beats[i], beats[i + 4 * args.bars]
    print(f"cut {a:.3f}-{b:.3f} s ({b - a:.2f} s, {args.bars} bars)")

    # Equal-power crossfade across the join.
    xf = int(rate * 0.25)
    ia, ib = int(a * rate), int(b * rate)
    t = np.linspace(0, np.pi / 2, 2 * xf)[:, None]
    join = audio[ia - xf:ia + xf] * np.cos(t) + audio[ib - xf:ib + xf] * np.sin(t)
    bed = np.concatenate([audio[:ia - xf], join, audio[ib + xf:]])

    n = int(args.length * rate)
    bed = bed[:n] if len(bed) >= n else np.concatenate([bed, np.zeros((n - len(bed), bed.shape[1]), "float32")])
    fade = int(args.fade_out * rate)
    bed[-fade:] *= np.linspace(1, 0, fade)[:, None] ** 2

    bed = pyln.normalize.loudness(bed, pyln.Meter(rate).integrated_loudness(bed), args.lufs)
    peak = np.abs(bed).max()
    if peak > 0.95:
        bed *= 0.95 / peak
    sf.write(args.out, bed.astype("float32"), rate, subtype="PCM_24")
    print(f"{args.out}: {len(bed) / rate:.2f} s, {rate} Hz, {args.lufs} LUFS")


if __name__ == "__main__":
    main()
