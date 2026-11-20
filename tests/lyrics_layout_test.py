"""Regression tests: lyric typography must fit the panel (no clipping, overflow or overlap) for short, medium and very
long lines, at different island sizes, display scalings and lyric-size settings. Run against `npm run dev`."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
SHORT = "Hold on"
MEDIUM = "Streetlights drifting past the glass tonight"
LONG = ("Counting down the miles we have left on this long and winding road that never seems to end, "
        "carrying every memory we ever made across the whole wide quiet city tonight")
WORD = "Supercalifragilisticexpialidocious-and-antidisestablishmentarianism-without-any-spaces-at-all-whatsoever"
LINES = [("before", "Opening line before everything"), ("short", SHORT), ("medium", MEDIUM), ("long", LONG), ("word", WORD), ("after", "Last line after")]
LRC = "".join(f"[00:{5*(i+1):02d}.00]{t}\n" for i, (_, t) in enumerate(LINES))
POS = {name: (5 * (i + 1) + 1) * 1000 for i, (name, _) in enumerate(LINES)}
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

R = """(sel) => { const e = document.querySelector(sel); if (!e) return null; const r = e.getBoundingClientRect();
  return {l:r.left,t:r.top,r:r.right,b:r.bottom,w:r.width,h:r.height,fs:parseFloat(getComputedStyle(e).fontSize),sw:e.scrollWidth,cw:e.clientWidth,sh:e.scrollHeight,ch:e.clientHeight}; }"""

def inside(a, outer, tol=1.5, what=""):
    assert a and outer, (what, a, outer)
    assert a["l"] >= outer["l"] - tol and a["r"] <= outer["r"] + tol and a["t"] >= outer["t"] - tol and a["b"] <= outer["b"] + tol, (what, a, outer)

def set_range(sp, testid, v):
    sp.evaluate("""([id, v]) => { const el = document.querySelector(`[data-testid="${id}"]`);
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, String(v)); el.dispatchEvent(new Event('input', {bubbles: true})); }""", [testid, v])

def setup(pg, pos_name, style="line"):
    pg.evaluate("l=>window.__harness.setLyricsReply({synced:l, plain:null, instrumental:false, source:'LRCLIB'})", LRC)
    pg.evaluate("p=>window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:p, status:'paused'})", POS[pos_name])
    pg.wait_for_timeout(500)

def open_tab(pg, tab):
    isl = pg.locator('[data-testid="island"]')
    isl.hover(); pg.wait_for_timeout(900)
    if isl.get_attribute("data-expanded") != "true":
        pg.evaluate("window.__harness.emitIslandToggle()"); pg.wait_for_timeout(900)
    pg.locator(f'[data-testid="tab-{tab}"]').click(); pg.wait_for_timeout(900)

def peek_ok(pg, name):
    isl = pg.evaluate(R, '[data-testid="island"]'); stage = pg.evaluate(R, ".music-stage")
    prog = pg.evaluate(R, '[data-testid="progress-bar"]'); titles = pg.evaluate(R, ".titles")
    now = pg.evaluate(R, '[data-testid="peek-now"]')
    assert now, "no current lyric"
    inside(stage, isl, what="stage in island"); inside(now, stage, what=f"{name}: current line in stage")
    for sel in ('[data-testid="peek-prev"]', ".peek-next"):
        r = pg.evaluate(R, sel)
        if r: inside(r, stage, what=f"{name}: {sel} in stage"); assert r["fs"] < now["fs"] + 0.01, (sel, r["fs"], now["fs"])
    assert stage["t"] >= titles["b"] - 1.5 and stage["b"] <= prog["t"] + 1.5, ("overlaps header or controls", stage, titles, prog)
    assert now["sw"] <= now["cw"] + 1, ("horizontal clipping", now)
    assert now["fs"] <= 18, now["fs"]
    k = isl["w"] / 540.0  # rects are in screen pixels, font sizes in layout pixels
    assert now["h"] / k <= 2 * now["fs"] * 1.3 + 2, ("more than two lines", now, k)
    return now

def lyrics_ok(pg, name):
    isl = pg.evaluate(R, '[data-testid="island"]'); vp = pg.evaluate(R, ".lyr-viewport"); head = pg.evaluate(R, ".lyrics-head")
    act = pg.evaluate(R, '[data-testid="lyric-active"]'); assert act, "no active line"
    inside(vp, isl, what="viewport in island")
    assert vp["t"] >= head["b"] - 1.5, ("viewport overlaps header", vp, head)
    # the active line (including its emphasis scale) stays inside the viewport, horizontally and vertically
    assert act["l"] >= vp["l"] - 1 and act["r"] <= vp["r"] + 1, (name, "active line clipped sideways", act, vp)
    assert act["t"] >= vp["t"] - 1 and act["b"] <= vp["b"] + 1, (name, "active line outside viewport", act, vp)
    assert act["fs"] <= 18 and act["sw"] <= act["cw"] + 2, act
    # every line that is drawn stays within the viewport's horizontal bounds (no sideways overflow)
    over = pg.evaluate("""() => { const vp = document.querySelector('.lyr-viewport').getBoundingClientRect();
      return [...document.querySelectorAll('.lyr-line')].filter(e => { const r = e.getBoundingClientRect(); return r.bottom > vp.top && r.top < vp.bottom && (r.left < vp.left - 1 || r.right > vp.right + 1); }).length; }""")
    assert over == 0, (name, "lines overflow sideways", over)
    return act

with sync_playwright() as p:
    b = p.chromium.launch()
    # (display scaling, island scales, lyric sizes, styles): the full matrix at 1x, a reduced one at the other densities
    for dsf, scales, sizes, styles in ((1, (1.0, 1.4, 0.8), (13, 32), ('line', 'letters')), (1.5, (1.0,), (13,), ('line',)), (2, (1.4,), (32,), ('letters',))):
        ctx = b.new_context(viewport={"width": 760, "height": 460}, device_scale_factor=dsf)
        errors = []
        pg = ctx.new_page(); pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]')
        sp = ctx.new_page(); sp.set_viewport_size({"width": 900, "height": 700}); sp.goto(BASE + "/settings.html"); sp.wait_for_selector('[data-testid="settings"]')
        sp.locator('[data-testid="page-music"]').click(); sp.wait_for_timeout(200)
        tag = f"dsf={dsf}"

        def move_pointer_away():
            pg.mouse.move(5, 450); pg.wait_for_timeout(1500)

        for scale in scales:
            sp.locator('[data-testid="page-appearance"]').click(); set_range(sp, 'scale', scale); sp.wait_for_timeout(300)
            sp.locator('[data-testid="page-music"]').click()
            for size in sizes:
                set_range(sp, 'lyrics-size', size); sp.wait_for_timeout(250)
                for style in styles:
                    sp.locator(f'[data-testid="lyrics-style-{style}"]').click(); sp.wait_for_timeout(200)
                    # the long unbreakable word is only expected to fit when words may be broken ("line" style); letters style keeps words whole
                    names = [n for n, _ in LINES] if style == "line" else [n for n, _ in LINES if n != "word"]
                    move_pointer_away(); setup(pg, "short")
                    for tab, fn in (("music", peek_ok), ("lyrics", lyrics_ok)):
                        open_tab(pg, tab)
                        for n in names:
                            pg.evaluate("p=>window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:p, status:'paused'})", POS[n]); pg.wait_for_timeout(550)
                            label = f"{tag} scale={scale} size={size} style={style} tab={tab} line={n}"
                            check(label, lambda fn=fn, n=n, label=label: fn(pg, label))
                        move_pointer_away()
            sp.locator('[data-testid="page-music"]').click()

        def rapid_updates():
            open_tab(pg, "music")
            for _ in range(3):
                for n, _t in LINES:
                    pg.evaluate("p=>window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:p, status:'playing'})", POS[n]); pg.wait_for_timeout(25)
            pg.evaluate("p=>window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:p, status:'paused'})", POS["long"]); pg.wait_for_timeout(900)
            peek_ok(pg, "after rapid updates")
            open_tab(pg, "lyrics"); lyrics_ok(pg, "after rapid updates")
            assert not errors, errors
        check(f"{tag}: rapid lyric updates leave a correct, error-free layout", rapid_updates)
        ctx.close()
    b.close()
print(f"{sum(results)}/{len(results)} lyric layout checks passed"); sys.exit(0 if all(results) else 1)
