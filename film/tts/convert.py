"""Piper's words in a human voice.

Piper pronounces Hungarian well and sounds like a machine; a voice converter keeps the words
and the timing and swaps the sound of the speaker. Takes a scene's sentences as narrate.py
wrote them and writes the same files, levelled the same way, to another folder. Both run in
the chatterbox environment:

  knn         kNN-VC (MIT; C:/Users/vasta/tools/knn-vc/repo): every moment of the sound is
              replaced by the nearest one in a long recording of a Hungarian reader, so the
              Hungarian sounds come from Hungarian speech
  chatterbox  ChatterboxVC (MIT): natural, but it works through a tokenizer that doesn't
              know Hungarian, and swallows sounds ("ajánlat" came out as "ajálak")

  C:/Users/vasta/tools/chatterbox/.venv/Scripts/python.exe -I film/tts/convert.py 0 \
      --source film/audio/vo-test/piper --out film/audio/vo-test/knn
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from narrate import PAUSE_S, REF, master  # noqa: E402

KNN = Path("C:/Users/vasta/tools/knn-vc")
# LibriVox, Apáthy István: Csöndes este (public domain): the reader the F5 voice was cloned from.
READER = REF / "librivox" / "msw038_15_csondeseste_apathy_gysz_64kb.mp3"
SKIP_S = 25  # the LibriVox announcement at each end
CHUNK_S = 20


def knn_converter(device):
    import torch
    import torchaudio

    sys.path.insert(0, str(KNN / "repo"))
    from hubconf import knn_vc

    model = knn_vc(pretrained=True, progress=False, prematched=True, device=device)
    cache = KNN / "reader.pt"
    if cache.exists():
        reader = torch.load(cache, map_location=device)
    else:
        audio, rate = sf.read(READER, dtype="float32", always_2d=True)
        speech = torch.from_numpy(audio.mean(axis=1)[SKIP_S * rate:-SKIP_S * rate].copy())
        speech = torchaudio.functional.resample(speech, rate, 16000)
        # In pieces: WavLM's memory grows with the square of the length.
        pieces = [p[None] for p in speech.split(CHUNK_S * 16000) if len(p) > 16000]
        reader = model.get_matching_set(pieces, vad_trigger_level=0)
        torch.save(reader, cache)
    print(f"reader: {len(reader)} frames", flush=True)

    def convert(path):
        audio, rate = sf.read(path, dtype="float32")
        wav = torchaudio.functional.resample(torch.from_numpy(audio.copy()), rate, 16000)
        query = model.get_features(wav[None])
        out = model.match(query, reader, topk=4, device=device)
        return out.cpu().numpy().astype("float64").reshape(-1), 16000

    return convert


def chatterbox_converter(device):
    from chatterbox.vc import ChatterboxVC

    model = ChatterboxVC.from_pretrained(device)

    def convert(path):
        wav = model.generate(audio=str(path), target_voice_path=str(REF / "zsorzs_ref.wav"))
        return wav.squeeze(0).cpu().numpy().astype("float64"), model.sr

    return convert


def soft_edges(line, rate):
    """A few milliseconds of fade at each end, so a sentence can't start or stop with a tick."""
    fade = int(rate * 0.008)
    line[:fade] *= np.linspace(0, 1, fade, dtype=line.dtype)
    line[-fade:] *= np.linspace(1, 0, fade, dtype=line.dtype)
    return line


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("scenes", help="comma-separated scene numbers")
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--engine", default="knn", choices=["knn", "chatterbox"])
    parser.add_argument("--device", default="cpu")
    args = parser.parse_args()

    convert = (knn_converter if args.engine == "knn" else chatterbox_converter)(args.device)
    args.out.mkdir(parents=True, exist_ok=True)

    for scene in args.scenes.split(","):
        clips, rate = [], None
        for path in sorted(args.source.glob(f"{scene}_*.wav"), key=lambda p: int(p.stem.split("_")[1])):
            audio, rate = convert(path)
            line = soft_edges(master(audio, rate), rate)
            sf.write(args.out / path.name, line, rate, subtype="PCM_16")
            clips += [line, np.zeros(int(rate * PAUSE_S), dtype="float32")]
            print(f"{path.stem}: {len(line) / rate:.1f} s", flush=True)
        joined = np.concatenate(clips[:-1])
        sf.write(args.out / f"{scene}.wav", joined, rate)
        meta = json.loads((args.source / f"{scene}.json").read_text(encoding="utf-8"))
        meta.update(engine=f"piper+{args.engine}", seconds=round(len(joined) / rate, 2))
        (args.out / f"{scene}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"scene {scene}: {meta['seconds']} s", flush=True)


if __name__ == "__main__":
    main()
