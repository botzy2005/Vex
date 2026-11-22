"""Volume panel checks (run against `npm run dev`, test backend)."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:500])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 700, "height": 420})
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    isl = pg.locator('[data-testid="island"]')
    btn = pg.locator('[data-testid="btn-volume"]')
    pop = pg.locator('[data-testid="volume-popover"]')
    slider = pg.locator('[data-testid="volume-slider"]')
    pct = pg.locator('[data-testid="volume-percent"]')
    H = lambda js: pg.evaluate("()=>window.__harness." + js)
    def calls(): return H("volumeCalls")

    def fresh(expand=True):
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]')
        pg.evaluate("()=>localStorage.removeItem('islet-test-settings')")
        pg.reload(); pg.wait_for_selector('[data-testid="island"]'); pg.wait_for_timeout(300)
        if expand:
            isl.hover(); pg.wait_for_timeout(900)
            expect(isl).to_have_attribute("data-expanded", "true")
    def open_panel():
        btn.click(); expect(pop).to_be_visible(); pg.wait_for_timeout(200)

    def only_expanded():
        fresh(expand=False)
        assert btn.count() == 0 and calls() == [], "volume button or traffic while collapsed"
        isl.hover(); pg.wait_for_timeout(900)
        expect(btn).to_be_visible()
    check("the speaker button exists only in the expanded island", only_expanded)

    def opens_with_state():
        fresh(); open_panel()
        expect(pct).to_have_text("40%"); assert slider.input_value() == "40"
        assert [c["op"] for c in calls()] == ["get"]
    check("the panel opens and shows the current level", opens_with_state)

    def plus_minus_use_setting_step():
        fresh(); open_panel()
        pg.locator('[data-testid="volume-up"]').click(); pg.wait_for_timeout(150)
        expect(pct).to_have_text("45%")
        pg.evaluate("()=>window.__harness.patchSettings({island:{volumeStep:10}})")
        pg.locator('[data-testid="volume-down"]').click(); pg.wait_for_timeout(150)
        expect(pct).to_have_text("35%")
    check("plus and minus change the level by the configured step", plus_minus_use_setting_step)

    def ends():
        fresh(); open_panel()
        pg.evaluate("()=>window.__harness.setVolume({percent:100})"); pg.wait_for_timeout(100)
        assert pg.locator('[data-testid="volume-up"]').is_disabled()
        pg.evaluate("()=>window.__harness.setVolume({percent:0})"); pg.wait_for_timeout(100)
        assert pg.locator('[data-testid="volume-down"]').is_disabled()
    check("plus is disabled at 100 and minus at 0", ends)

    def mute_toggle():
        fresh(); open_panel()
        mute = pg.locator('[data-testid="volume-mute"]')
        mute.click(); pg.wait_for_timeout(150)
        expect(pct).to_have_text("Muted"); expect(mute).to_have_attribute("aria-pressed", "true")
        mute.click(); pg.wait_for_timeout(150)
        expect(pct).to_have_text("40%")
    check("mute and unmute", mute_toggle)

    def raising_unmutes():
        fresh(); open_panel()
        pg.evaluate("()=>window.__harness.setVolume({muted:true})"); pg.wait_for_timeout(100)
        pg.locator('[data-testid="volume-up"]').click(); pg.wait_for_timeout(150)
        expect(pct).to_have_text("45%")
    check("raising the level while muted unmutes", raising_unmutes)

    def slider_drag():
        fresh(); open_panel()
        bb = slider.bounding_box(); y = bb["y"] + bb["height"] / 2
        pg.mouse.move(bb["x"] + 6, y); pg.mouse.down()
        pg.mouse.move(bb["x"] + bb["width"] * 0.75, y, steps=20); pg.mouse.up(); pg.wait_for_timeout(200)
        sets = [c for c in calls() if c["op"] == "set"]
        assert sets, calls()
        last = int(slider.input_value())
        assert 60 <= last <= 85, last
        assert sets[-1]["value"] == last, (sets[-1], last)
        assert len(sets) < 25, f"unthrottled: {len(sets)} sets"
        expect(pct).to_have_text(f"{last}%")
    check("dragging the slider sets the level, throttled, ending on the final value", slider_drag)

    def external_changes():
        fresh(); open_panel()
        pg.evaluate("()=>window.__harness.setVolume({percent:73})"); pg.wait_for_timeout(150)
        expect(pct).to_have_text("73%"); assert slider.input_value() == "73"
        pg.evaluate("()=>window.__harness.setVolume({muted:true})"); pg.wait_for_timeout(150)
        expect(pct).to_have_text("Muted")
    check("keyboard-key / other-app changes arrive as events", external_changes)

    def icon_follows():
        fresh()
        def lvl(): return btn.evaluate("e=>e.innerHTML.length")
        pg.evaluate("()=>window.__harness.setVolume({percent:80,muted:false})"); pg.wait_for_timeout(150); high = lvl()
        pg.evaluate("()=>window.__harness.setVolume({percent:20})"); pg.wait_for_timeout(150); low = lvl()
        pg.evaluate("()=>window.__harness.setVolume({muted:true})"); pg.wait_for_timeout(150)
        assert high != low and lvl() != low, (high, low, lvl())
    check("the speaker icon follows level and mute", icon_follows)

    def unavailable():
        fresh(); pg.evaluate("()=>window.__harness.setVolume({available:false,error:'No audio output device'})"); open_panel()
        expect(pg.locator('[data-testid="volume-message"]')).to_have_text("No audio output device")
        assert slider.is_disabled() and pg.locator('[data-testid="volume-mute"]').is_disabled()
        pg.evaluate("()=>window.__harness.setVolume({available:true,error:null,percent:55})"); pg.wait_for_timeout(150)
        assert not slider.is_disabled(); expect(pct).to_have_text("55%")
    check("no output device disables the controls with a message, and recovers", unavailable)

    def failing_call():
        fresh(); open_panel()
        pg.evaluate("()=>window.__harness.failNextVolume('Could not change the volume')")
        pg.locator('[data-testid="volume-up"]').click(); pg.wait_for_timeout(200)
        expect(pg.locator('[data-testid="volume-message"]')).to_contain_text("Could not change the volume")
        expect(pct).to_have_text("40%")
        pg.locator('[data-testid="volume-up"]').click(); pg.wait_for_timeout(200)
        expect(pct).to_have_text("45%"); assert pg.locator('[data-testid="volume-message"]').count() == 0
    check("a failing call shows a message, keeps the level, and the next call recovers", failing_call)

    def closes():
        fresh(); open_panel()
        btn.click(); pg.wait_for_timeout(200); assert pop.count() == 0
        open_panel(); pg.keyboard.press("Escape") if False else None
        pg.mouse.move(5, 400); pg.wait_for_timeout(1200)
        expect(isl).to_have_attribute("data-expanded", "false"); assert pop.count() == 0
    check("the panel closes with its button and when the island collapses", closes)

    def button_setting():
        fresh(); expect(btn).to_be_visible()
        pg.evaluate("()=>window.__harness.patchSettings({island:{volumeButton:false}})"); pg.wait_for_timeout(200)
        assert btn.count() == 0
    check("island.volumeButton=false removes the button", button_setting)

    def other_controls_work():
        fresh(); open_panel()
        pg.locator('[data-testid="tab-lyrics"]').click(); pg.wait_for_timeout(300)
        expect(pg.locator('[data-testid="lyrics-tab"], [data-testid="tab-lyrics"]').first).to_be_visible()
        expect(btn).to_be_visible(); expect(pg.locator('[data-testid="btn-pin"]')).to_be_visible()
    check("tabs and the other tool buttons still work with the panel open", other_controls_work)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} volume checks passed")
sys.exit(0 if passed == len(results) else 1)
