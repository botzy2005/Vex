"""VEX 2.0 island surface checks (run against `npm run dev`, test backend)."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
SVG = ("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'>"
       "<rect width='300' height='300' fill='%23d86aa5'/></svg>")
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:400])

with sync_playwright() as p:
    b = p.chromium.launch(); pg = b.new_context(viewport={"width": 700, "height": 420}).new_page()
    pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]')
    isl = pg.locator('[data-testid="island"]')
    pg.evaluate("window.__harness.setLyricsReply({synced:'[00:01.00]first line\\n[00:05.00]second line\\n[00:09.00]third line', plain:null, instrumental:false, source:'LRCLIB'})")
    pg.evaluate("a=>window.__harness.setSession({title:'T', artist:'A', durationMs:200000, positionMs:6000, status:'playing', artworkUrl:a})", SVG)
    pg.wait_for_timeout(700)
    isl.hover(); pg.wait_for_timeout(1200)

    def backdrop_on_music():
        expect(isl).to_have_attribute("data-media", "true")
        expect(pg.locator('[data-testid="media-backdrop"]')).to_have_attribute("data-on", "true")
        assert abs(isl.bounding_box()["height"] - 246) < 2
    check("music tab: the cover becomes the (scrimmed) surface", backdrop_on_music)

    def peek_has_three_lines():
        expect(pg.locator('[data-testid="peek-prev"]')).to_be_visible()
        expect(pg.locator('[data-testid="peek-now"]')).to_be_visible()
    check("music tab: previous / current lyric line hierarchy is rendered", peek_has_three_lines)

    def lyrics_tab_layout():
        pg.locator('[data-testid="tab-lyrics"]').click(); pg.wait_for_timeout(700)
        expect(pg.locator('[data-testid="lyrics-tab"] .lyrics-head')).to_be_visible()
        expect(isl).to_have_attribute("data-media", "true")
        assert abs(isl.bounding_box()["height"] - 274) < 2
        assert pg.locator('[data-testid="lyrics-tab"] [data-testid="btn-toggle"]').count() == 1
        size = pg.evaluate("parseFloat(getComputedStyle(document.querySelector('.lyr-line')).fontSize)")
        assert size <= 14.5, size  # default lyric size is calmer than before (15px, scaled)
    check("lyrics tab: header with cover and controls above the lyric stage, backdrop kept", lyrics_tab_layout)

    def backdrop_off_elsewhere():
        for t in ("assistant", "clipboard", "screenshot"):
            pg.locator(f'[data-testid="tab-{t}"]').click(); pg.wait_for_timeout(500)
            expect(isl).to_have_attribute("data-media", "false")
    check("other tabs keep the plain near-solid surface", backdrop_off_elsewhere)

    def opacity_floor():
        a = pg.evaluate("""() => { const s = document.querySelector('.stage'); s.style.setProperty('--glass','0.4');
          const bg = getComputedStyle(document.querySelector('[data-testid=island]')).backgroundImage;
          return bg; }""")
        import re
        alphas = [float(x) for x in re.findall(r"rgba\([^)]*?,\s*([0-9.]+)\)", a)]
        assert alphas and min(alphas) >= 0.88, (alphas, a)
    check("surface never drops below ~90% opaque, even at the lowest glass setting", opacity_floor)

    def collapses_without_backdrop():
        pg.mouse.move(5, 410); pg.wait_for_timeout(1800)
        expect(isl).to_have_attribute("data-media", "false")
        expect(pg.locator('[data-testid="media-backdrop"]')).to_have_attribute("data-on", "false")
    check("collapsed island is plain again (no backdrop)", collapses_without_backdrop)

    sp = pg.context.new_page(); sp.set_viewport_size({"width": 860, "height": 620})
    sp.goto(BASE + "/settings.html"); sp.wait_for_selector('[data-testid="appearance-studio"]'); sp.wait_for_timeout(400)
    stored = lambda: __import__("json").loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["appearance"]

    def studio_preview_and_default_preset():
        expect(sp.locator('[data-testid="island-preview"]')).to_be_visible()
        expect(sp.locator('[data-testid="preset-midnight"]')).to_have_attribute("aria-pressed", "true")
    check("studio: live preview is shown and the default look is the active preset", studio_preview_and_default_preset)

    def preset_applies_and_preview_follows():
        sp.locator('[data-testid="preset-notch"]').click(); sp.wait_for_timeout(400)
        a = stored(); assert a["shape"] == "notch" and a["cornerRadius"] == 20, a
        expect(sp.locator('[data-testid="island-preview"]')).to_have_attribute("data-shape", "notch")
        assert sp.evaluate("getComputedStyle(document.querySelector('[data-testid=preview-island]')).borderTopLeftRadius") == "0px"
        expect(sp.locator('[data-testid="preset-notch"]')).to_have_attribute("aria-pressed", "true")
        sp.locator('[data-testid="preset-daylight"]').click(); sp.wait_for_timeout(400)
        assert stored()["theme"] == "light"
        expect(sp.locator('[data-testid="island-preview"]')).to_have_attribute("data-theme", "light")
    check("studio: a preset applies its settings and the preview follows", preset_applies_and_preview_follows)

    def tweak_clears_active_preset():
        sp.locator('[data-testid="radius"]').fill("31"); sp.wait_for_timeout(300)
        expect(sp.locator('[data-testid="preset-daylight"]')).to_have_attribute("aria-pressed", "false")
        r = sp.evaluate("getComputedStyle(document.querySelector('[data-testid=preview-island]')).borderTopLeftRadius")
        assert abs(float(r[:-2]) - 31) < 0.1, r
    check("studio: fine-tuning a control leaves the preset and the preview radius updates", tweak_clears_active_preset)

    def custom_accent_reaches_preview():
        sp.locator('[data-testid="accent-source-custom"]').click(); sp.locator('[data-testid="accent-color"]').fill("#ff8800"); sp.wait_for_timeout(300)
        acc = sp.evaluate("getComputedStyle(document.querySelector('[data-testid=island-preview]')).getPropertyValue('--accent').trim()")
        assert acc.lower() == "#ff8800", acc
    check("studio: the chosen accent colour shows in the preview", custom_accent_reaches_preview)
    b.close()
print(f"{sum(results)}/{len(results)} redesign checks passed"); sys.exit(0 if all(results) else 1)
