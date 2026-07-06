# Génère wix/avion.ico (tailles 32 et 256, PNG dans conteneur ICO) depuis les
# rects du sprite (mêmes coordonnées/couleurs que src/sprite.rs::PLANE_RECTS).
import struct, zlib

RECTS = [  # (x, y, w, h, rgb) — coordonnées du rig, avion dans (716..856, 60..152)
    (716, 60, 16, 34, 0xC9D4E0), (716, 60, 16, 6, 0xE6EDF4),
    (730, 82, 92, 10, 0xE23B3B), (720, 92, 112, 10, 0xE23B3B),
    (720, 102, 112, 10, 0xC62F2F), (730, 112, 92, 10, 0xC62F2F),
    (786, 84, 14, 8, 0xBFE3FF), (804, 84, 14, 8, 0xBFE3FF),
    (738, 122, 72, 10, 0xC9D4E0), (738, 122, 72, 4, 0xE6EDF4),
    (754, 132, 4, 12, 0x3B3F47), (794, 132, 4, 12, 0x3B3F47),
    (746, 144, 20, 8, 0x2B2F36), (786, 144, 20, 8, 0x2B2F36),
    (832, 92, 12, 20, 0x2B2F36), (840, 94, 10, 10, 0xFFCF3F),
    (850, 74, 6, 56, 0x3B3F47),
]
OX, OY, W, H = 716, 60, 140, 92  # boîte englobante de l'avion


def render(size):
    """RGBA size×size : avion mis à l'échelle (plus proche voisin), centré."""
    px = bytearray(size * size * 4)
    scale = size / W
    y_off = int((size - H * scale) / 2)
    for (x, y, w, h, c) in RECTS:
        x0 = int((x - OX) * scale)
        y0 = int((y - OY) * scale) + y_off
        x1 = max(x0 + 1, int((x - OX + w) * scale))
        y1 = max(y0 + 1, int((y - OY + h) * scale) + y_off)
        r, g, b = (c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF
        for yy in range(max(0, y0), min(size, y1)):
            for xx in range(max(0, x0), min(size, x1)):
                i = (yy * size + xx) * 4
                px[i : i + 4] = bytes((r, g, b, 0xFF))
    return bytes(px)


def png(size, rgba):
    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))

    raw = b"".join(b"\x00" + rgba[y * size * 4 : (y + 1) * size * 4] for y in range(size))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


images = [(s, png(s, render(s))) for s in (32, 256)]
out = struct.pack("<HHH", 0, 1, len(images))
offset = 6 + 16 * len(images)
entries, blobs = b"", b""
for s, blob in images:
    entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(blob), offset)
    offset += len(blob)
    blobs += blob
with open(r"wix/avion.ico", "wb") as f:
    f.write(out + entries + blobs)
print("ok", sum(len(b) for _, b in images), "octets de PNG")
