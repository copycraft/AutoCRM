"""Voice the film's narration, one sentence per file.

Reads the narration lines (the `> ` quotes) of film/script.md, scene by scene, and writes
film/audio/vo/<scene>_<sentence>.wav plus film/audio/vo/<scene>.wav with short pauses
between sentences. A sentence that comes out wrong is re-made on its own with --only.

The voice is cloned from a short Hungarian reference clip (VOICES) by one of two engines,
each in its own environment under C:/Users/vasta/tools:

  f5    F5-TTS fine-tuned on Hungarian (Maxdorger29/f5-tts-hungarian, CC BY-NC 4.0)
        C:/Users/vasta/tools/f5-tts/.venv/Scripts/python.exe -I film/tts/narrate.py --scenes 0
        (on the Arc GPU: tools/f5-tts-xpu/.venv and --device xpu)
  qwen  Qwen3-TTS 1.7B Base; Hungarian is not on its list, so the language is "Auto"
        C:/Users/vasta/tools/qwen-tts/.venv/Scripts/python.exe -I film/tts/narrate.py --engine qwen
  piper Piper's Hungarian male voice, hu_HU-imre-medium, with no cloning
        C:/Users/vasta/tools/piper/.venv/Scripts/python.exe -I film/tts/narrate.py --engine piper
"""

import argparse
import json
import re
import time
from pathlib import Path

import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "script.md"
OUT = ROOT / "audio" / "vo"
REF = Path("C:/Users/vasta/tools/outetts/ref")
PIPER = Path("C:/Users/vasta/tools/piper/voices/hu_HU-imre-medium.onnx")

# Reference voices: a clip under 15 s and exactly what is said in it.
VOICES = {
    # LibriVox reader "Zsorzs", Apáthy István: Csöndes este (public domain), 54.85-64.75 s.
    "male": (
        REF / "zsorzs_ref.wav",
        "A béke és a szabadság órája ez. A nappali gyötrelmek véget értek, s az éjszaka rémei még nincsenek itt.",
    ),
    # CSS10 Hungarian (CC0), Gárdonyi Géza: Egri csillagok.
    "female": (
        REF / "css10_20.wav",
        "Az a kis falu, ahova a két gyermek való, a Mecsek egy völgyében rejtőzködik. Valami harminc vályogház meg egy nagy kőház mindössze.",
    ),
}

# Spoken forms for what a Hungarian reader says differently from how it is written.
SPOKEN = {
    "AutoCRM": "autó-cé-er-em",
    "PDF": "pédéef",
    "MNB": "emenbé",
    "NAV": "náv",
    "MEO": "meó",
    "2D": "kétdé",
    "3D": "háromdé",
}

PAUSE_S = 0.35


def narration() -> dict[int, str]:
    """Scene number -> its narration text."""
    scenes, current = {}, None
    for line in SCRIPT.read_text(encoding="utf-8").splitlines():
        heading = re.match(r"^## (\d+) · ", line)
        if heading:
            current = int(heading.group(1))
        elif current is not None and line.startswith("> "):
            scenes[current] = line[2:].strip()
    return scenes


def spoken(text: str) -> str:
    for written, said in SPOKEN.items():
        text = re.sub(rf"\b{re.escape(written)}", said, text)
    return text


def master(audio: np.ndarray, rate: int, lufs: float = -16.0) -> np.ndarray:
    """Even out a line and set its level.

    The level is first evened out (about 2.5:1 on the speech, gain kept within 0.4-3x,
    smoothed over 80 ms), then the line is brought to `lufs` and its peaks held under
    -1 dBFS.
    """
    import pyloudnorm as pyln

    hop = int(rate * 0.01)
    frames = len(audio) // hop
    env = np.array([np.sqrt(np.mean(audio[i * hop:(i + 3) * hop] ** 2)) for i in range(frames)]) + 1e-9
    voiced = env > env.max() * 0.08
    target = np.median(env[voiced])
    gain = np.where(voiced, np.clip((target / env) ** 0.6, 0.4, 3.0), np.nan)
    known = np.flatnonzero(~np.isnan(gain))
    gain = np.interp(np.arange(frames), known, gain[known])
    gain = np.convolve(gain, np.ones(8) / 8, mode="same")
    audio = audio * np.interp(np.arange(len(audio)), np.arange(frames) * hop + hop / 2, gain)

    audio = pyln.normalize.loudness(audio, pyln.Meter(rate).integrated_loudness(audio), lufs)

    # Peak hold: the gain each sample needs, held over 5 ms and smoothed, so peaks bend
    # instead of clipping.
    ceiling = 10 ** (-1 / 20)
    need = np.minimum(1.0, ceiling / (np.abs(audio) + 1e-9))
    win = max(1, int(rate * 0.005))
    held = np.array([need[max(0, i - win):i + win].min() for i in range(0, len(need), win)])
    held = np.convolve(np.repeat(held, win)[: len(audio)], np.ones(win) / win, mode="same")
    return (audio * held).clip(-ceiling, ceiling).astype("float32")


def sentences(text: str) -> list[str]:
    return [s.strip() for s in re.split(r"(?<=[.!?])\s+", text) if s.strip()]


def qwen_engine(voice: str):
    """Qwen3-TTS 1.7B Base, cloning the reference voice. Returns say(text, seed) -> (audio, rate)."""
    import torch
    from huggingface_hub import snapshot_download
    from qwen_tts import Qwen3TTSModel

    # The downloaded folder, not the repo id: given an id, the loader asks the Hub even offline.
    local = snapshot_download("Qwen/Qwen3-TTS-12Hz-1.7B-Base", local_files_only=True)
    model = Qwen3TTSModel.from_pretrained(local, device_map="cpu", dtype=torch.float32)
    path, transcript = VOICES[voice]
    ref, ref_rate = sf.read(path, dtype="float32")
    prompt = model.create_voice_clone_prompt(ref_audio=(ref, ref_rate), ref_text=transcript)

    def say(text, seed):
        torch.manual_seed(seed)
        wavs, rate = model.generate_voice_clone(text=text, language="Auto", voice_clone_prompt=prompt)
        return np.asarray(wavs[0], dtype="float64").reshape(-1), rate

    return say


def piper_engine():
    """Piper's Hungarian male voice, which needs no reference clip. Returns say(text, seed) -> (audio, rate)."""
    from piper import PiperVoice

    model = PiperVoice.load(str(PIPER))

    def say(text, seed):
        chunks = list(model.synthesize(text))
        return np.concatenate([c.audio_float_array for c in chunks]).astype("float64"), chunks[0].sample_rate

    return say


# The Hungarian F5-TTS reads only these characters (its vocab.txt), all lowercase.
F5_CHARS = set(" !'(),-.:;?aábcdeéfghiíjklmnoóöőpqrstuúüűvwxyz0123456789")


def f5_text(text: str) -> str:
    text = text.lower().replace("–", ",").replace("—", ",").replace("·", ",").replace("„", "").replace("”", "")
    return re.sub(r"\s+", " ", "".join(ch for ch in text if ch in F5_CHARS)).strip()


def f5_engine(voice: str, steps: int = 32, device: str = "cpu"):
    """F5-TTS fine-tuned on Hungarian, cloning the reference voice. Returns say(text, seed) -> (audio, rate)."""
    import torch
    import torchaudio

    # torchaudio 2.11 reads audio only through TorchCodec (FFmpeg); soundfile does it here.
    def load(fp, **kwargs):
        data, rate = sf.read(str(fp), dtype="float32", always_2d=True)
        return torch.from_numpy(data.T.copy()), rate

    torchaudio.load = load
    from f5_tts.api import F5TTS
    from huggingface_hub import hf_hub_download

    repo = "Maxdorger29/f5-tts-hungarian"
    model = F5TTS(
        model="F5TTS_v1_Base",
        ckpt_file=hf_hub_download(repo, "model_last_final.safetensors"),
        vocab_file=hf_hub_download(repo, "vocab.txt"),
        device=device,
        use_ema=True,
    )
    # The model card's cure for a garbled first few hundred milliseconds: end the reference
    # with 0.6 s of silence, then trim any quiet lead-in from what comes out.
    path, transcript = VOICES[voice]
    ref, ref_rate = sf.read(path, dtype="float32")
    padded = Path("C:/Users/vasta/tools/f5-tts") / f"ref_{voice}.wav"
    sf.write(padded, np.concatenate([ref, np.zeros(int(ref_rate * 0.6), dtype="float32")]), ref_rate)
    ref_text = f5_text(transcript)

    def say(text, seed):
        wav, rate, _ = model.infer(ref_file=str(padded), ref_text=ref_text, gen_text=f5_text(text), seed=seed, nfe_step=steps)
        wav = np.asarray(wav, dtype="float64").reshape(-1)
        hop = int(rate * 0.01)
        head = [np.sqrt(np.mean(wav[i:i + hop] ** 2)) for i in range(0, min(len(wav), int(rate * 0.4)) - hop, hop)]
        start = next((i for i, e in enumerate(head) if e > max(head) * 0.15), 0)
        return wav[max(0, start - 1) * hop:], rate

    return say


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenes", default="all", help="comma-separated scene numbers, or all")
    parser.add_argument("--only", help="re-make one sentence, e.g. 5_3")
    parser.add_argument("--engine", default="f5", choices=["f5", "qwen", "piper"])
    parser.add_argument("--out", type=Path, default=OUT, help="folder to write to, for trying a voice out")
    parser.add_argument("--voice", default="male", choices=sorted(VOICES))
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--steps", type=int, default=32, help="F5 sampling steps: fewer is faster, and rougher")
    parser.add_argument("--device", default="cpu", help="F5 device: cpu, or xpu for the Arc GPU (with the f5-tts-xpu environment)")
    parser.add_argument("--say", help="voice this text instead of the script (with --name)")
    parser.add_argument("--name", default="line", help="file name for --say")
    args = parser.parse_args()

    if args.engine == "f5":
        say = f5_engine(args.voice, args.steps, args.device)
    else:
        say = qwen_engine(args.voice) if args.engine == "qwen" else piper_engine()
    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    if args.say:
        scenes, wanted = {args.name: args.say}, [args.name]
    else:
        scenes = narration()
        wanted = sorted(scenes) if args.scenes == "all" else [int(s) for s in args.scenes.split(",")]
        if args.only:
            wanted = [int(args.only.split("_")[0])]

    for scene in wanted:
        parts = sentences(scenes[scene])
        for n, sentence in enumerate(parts, 1):
            name = f"{scene}_{n}"
            if args.only and name != args.only:
                continue
            started = time.time()
            audio, rate = say(spoken(sentence), args.seed + n)
            line = master(audio, rate)
            sf.write(out / f"{name}.wav", line, rate, subtype="PCM_16")
            took = time.time() - started
            print(f"{name}: {len(line) / rate:.1f} s in {took:.0f} s ({args.engine}): {sentence}", flush=True)

        # The scene file: every sentence in order, with a short pause between them.
        clips, rate = [], None
        for n in range(1, len(parts) + 1):
            path = out / f"{scene}_{n}.wav"
            if not path.exists():
                break
            audio, rate = sf.read(path, dtype="float32")
            clips += [audio, np.zeros(int(rate * PAUSE_S), dtype="float32")]
        else:
            joined = np.concatenate(clips[:-1])
            sf.write(out / f"{scene}.wav", joined, rate)
            meta = {"scene": scene, "engine": args.engine, "voice": args.voice, "seconds": round(len(joined) / rate, 2), "sentences": parts}
            (out / f"{scene}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding="utf-8")
            print(f"scene {scene}: {meta['seconds']} s", flush=True)


if __name__ == "__main__":
    main()
