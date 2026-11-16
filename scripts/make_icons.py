"""Generates the VEX app icons (src-tauri/icons). Run: python3 scripts/make_icons.py  (needs Pillow)."""
from PIL import Image, ImageDraw, ImageFilter
import os

S = 1024
OUT = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")

def lerp(a, b, t): return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

def master():
    # background: near-black rounded square with a faint vertical gradient
    bg = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        gd.line((0, y, S, y), fill=lerp((26, 26, 34), (4, 4, 6), y / S) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((24, 24, S - 24, S - 24), radius=232, fill=255)
    bg.paste(grad, (0, 0), mask)
    # hairline edge
    edge = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(edge).rounded_rectangle((24, 24, S - 24, S - 24), radius=232, outline=(255, 255, 255, 34), width=6)
    bg = Image.alpha_composite(bg, edge)

    # the V as a thick round-capped polyline
    w = 150
    pts = [(268, 292), (512, 744), (756, 292)]
    vmask = Image.new("L", (S, S), 0)
    vd = ImageDraw.Draw(vmask)
    vd.line(pts, fill=255, width=w, joint="curve")
    for x, y in (pts[0], pts[2]):
        vd.ellipse((x - w // 2, y - w // 2, x + w // 2, y + w // 2), fill=255)
    vd.ellipse((pts[1][0] - w // 2, pts[1][1] - w // 2, pts[1][0] + w // 2, pts[1][1] + w // 2), fill=255)

    # diagonal cyan -> violet gradient
    vg = Image.new("RGBA", (S, S))
    px = vg.load()
    for y in range(S):
        for x in range(S):
            t = min(1, max(0, (x + y - 400) / 900))
            px[x, y] = lerp((72, 224, 255), (150, 96, 255), t) + (255,)
    # soft glow behind the V
    glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    glow.paste(vg, (0, 0), vmask.filter(ImageFilter.GaussianBlur(40)))
    glow.putalpha(glow.getchannel("A").point(lambda a: int(a * 0.45)))
    bg = Image.alpha_composite(bg, glow)
    layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    layer.paste(vg, (0, 0), vmask)
    return Image.alpha_composite(bg, layer)

def main():
    m = master()
    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
        m.resize((size, size), Image.LANCZOS).save(os.path.join(OUT, name))
    m.resize((512, 512), Image.LANCZOS).save(os.path.join(OUT, "icon.png"))
    m.save(os.path.join(OUT, "icon.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    # preview strip for review
    strip = Image.new("RGBA", (512 + 256 + 64 + 32 + 16 + 60, 512), (59, 66, 82, 255))
    x = 10
    for sz in (512, 256, 64, 32, 16):
        strip.paste(m.resize((sz, sz), Image.LANCZOS), (x, 0), m.resize((sz, sz), Image.LANCZOS)); x += sz + 10
    strip.save(os.path.join(os.path.dirname(__file__), "..", "icon-preview.png"))

main()
