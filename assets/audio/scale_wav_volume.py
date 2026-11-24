#!/usr/bin/env python3
"""Scale the volume of WAV files in place.

    python3 assets/audio/scale_wav_volume.py 0.5 assets/audio/tool/laser_machine_pistol_*.wav

Multiplies every sample in the `data` chunk by FACTOR (0.5 is half as loud,
about -6 dB) and leaves every other byte, including metadata chunks such as
JUNK or BWBM, untouched. Handles PCM 8/16/24/32-bit and 32-bit float.
Each run scales again, so run it once per change. Writes a temporary file
beside each WAV and renames it over the original only when it succeeds.
"""
import os
import struct
import sys


def chunks(data):
    """Yield (id, payload_offset, size) for each RIFF chunk after the header."""
    offset = 12
    while offset + 8 <= len(data):
        chunk_id = data[offset:offset + 4]
        size = struct.unpack_from("<I", data, offset + 4)[0]
        yield chunk_id, offset + 8, size
        offset += 8 + size + (size & 1)


def scale_samples(samples, fmt, bits, factor):
    if fmt == 3 and bits == 32:
        count = len(samples) // 4
        values = struct.unpack(f"<{count}f", samples[:count * 4])
        return struct.pack(f"<{count}f", *(v * factor for v in values)) + samples[count * 4:]
    if fmt != 1:
        raise ValueError(f"unsupported WAV format {fmt} ({bits}-bit)")
    width = bits // 8
    out = bytearray(samples)
    if width == 1:
        # 8-bit PCM is unsigned around 128.
        for i, v in enumerate(samples):
            out[i] = max(0, min(255, round((v - 128) * factor) + 128))
        return bytes(out)
    if width not in (2, 3, 4):
        raise ValueError(f"unsupported PCM width {bits} bits")
    low, high = -(1 << (bits - 1)), (1 << (bits - 1)) - 1
    for i in range(0, len(samples) - width + 1, width):
        v = int.from_bytes(samples[i:i + width], "little", signed=True)
        v = max(low, min(high, round(v * factor)))
        out[i:i + width] = v.to_bytes(width, "little", signed=True)
    return bytes(out)


def scale_file(path, factor):
    data = bytearray(open(path, "rb").read())
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError("not a RIFF/WAVE file")
    fmt = bits = None
    for chunk_id, start, size in chunks(data):
        if chunk_id == b"fmt ":
            fmt, _channels, _rate, _byte_rate, _align, bits = struct.unpack_from("<HHIIHH", data, start)
            if fmt == 0xFFFE:  # WAVE_FORMAT_EXTENSIBLE: the subformat's first two bytes
                fmt = struct.unpack_from("<H", data, start + 24)[0]
        elif chunk_id == b"data":
            if fmt is None:
                raise ValueError("data chunk before fmt chunk")
            data[start:start + size] = scale_samples(bytes(data[start:start + size]), fmt, bits, factor)
            temporary = path + ".scaling.tmp"
            with open(temporary, "wb") as out:
                out.write(data)
            os.replace(temporary, path)
            return fmt, bits
    raise ValueError("no data chunk")


def main(argv):
    if len(argv) < 3:
        print(__doc__.strip())
        return 2
    factor = float(argv[1])
    if not 0.0 <= factor <= 4.0:
        print("factor must be between 0 and 4")
        return 2
    failed = False
    for path in argv[2:]:
        try:
            fmt, bits = scale_file(path, factor)
            print(f"{path}: scaled by {factor} ({'float' if fmt == 3 else 'PCM'} {bits}-bit)")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}")
            failed = True
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
