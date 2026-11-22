"""Auto-Hide checks (run against `npm run dev`, test backend)."""
import sys, json
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
    stage = pg.locator(".stage")
    isl = pg.locator('[data-testid="island"]')
    strip = pg.locator('[data-testid="edge-strip"]')
    H = lambda js: pg.evaluate("()=>window.__harness." + js)

    def fresh(auto=True, delay=300):
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]')
        pg.evaluate("()=>localStorage.removeItem('islet-test-settings')")
        pg.reload(); pg.wait_for_selector('[data-testid="island"]'); pg.wait_for_timeout(300)
        pg.mouse.move(5, 400)
        if auto:
            pg.evaluate("d=>window.__harness.patchSettings({island:{visibility:'auto',autoHideDelayMs:d}})", delay)
    def phase(): return stage.get_attribute("data-hide-phase")
    def hidden_calls(): return H("hiddenCalls")

    def default_is_always():
        fresh(auto=False); pg.wait_for_timeout(1500)
        assert phase() == "visible" and hidden_calls() == []
        assert json.loads(pg.evaluate("localStorage.getItem('islet-test-settings')") or "{}").get("island", {}).get("visibility", "always") == "always"
    check("Always Visible is the default and never hides", default_is_always)

    def always_no_timers():
        fresh(auto=False)
        pg.evaluate("()=>{window.__t=0;const o=window.setTimeout;window.setTimeout=function(){window.__t++;return o.apply(this,arguments)}}")
        pg.wait_for_timeout(1500)
        assert pg.evaluate("()=>window.__t") == 0, "timers scheduled while visible and idle in Always mode"
    check("no timers while visible and idle in Always mode", always_no_timers)

    def auto_hides_after_delay():
        fresh(); assert phase() == "visible"
        pg.wait_for_timeout(1200)
        assert phase() == "hidden", phase()
        assert hidden_calls()[-1] == {"hidden": True, "applied": True}
        expect(strip).to_be_attached()
        assert float(isl.evaluate("e=>getComputedStyle(e).opacity")) == 0
    check("Auto-Hide hides an idle island after the delay and shows the edge strip", auto_hides_after_delay)

    def strip_reveals():
        fresh(); pg.wait_for_timeout(1200); assert phase() == "hidden"
        strip.hover(); pg.wait_for_timeout(500)
        assert phase() == "visible", phase()
        assert hidden_calls()[-1]["hidden"] is False
        expect(strip).to_have_count(0)
    check("pointer on the top-edge strip reveals the island", strip_reveals)

    def stays_while_hovered():
        fresh(delay=300); pg.wait_for_timeout(1200); strip.hover(); pg.wait_for_timeout(300)
        isl.hover(); pg.wait_for_timeout(2000)
        assert phase() == "visible", phase()
    check("stays visible while the pointer is over it", stays_while_hovered)

    def hides_again_after_leaving():
        fresh(delay=300); pg.wait_for_timeout(1200); strip.hover(); pg.wait_for_timeout(300)
        pg.mouse.move(5, 400); pg.wait_for_timeout(1800)
        assert phase() == "hidden", phase()
    check("hides again after the pointer leaves", hides_again_after_leaving)

    def no_flicker():
        fresh(delay=200); pg.wait_for_timeout(1200); strip.hover(); pg.wait_for_timeout(100)
        pg.mouse.move(5, 400); pg.wait_for_timeout(250)
        assert phase() == "visible", "hid before the minimum visible time: " + phase()
    check("a reveal stays up at least the minimum visible time (no flicker)", no_flicker)

    def pinned_holds():
        fresh(delay=300)
        pg.evaluate("()=>window.__harness.emitIslandToggle()"); pg.wait_for_timeout(300)
        expect(isl).to_have_attribute("data-pinned", "true")
        pg.mouse.move(5, 400); pg.wait_for_timeout(1500)
        assert phase() == "visible", phase()
    check("stays visible while expanded and pinned", pinned_holds)

    def assistant_busy_holds():
        fresh(delay=300)
        pg.evaluate("()=>window.__harness.emitAssistant('thinking')"); pg.wait_for_timeout(1500)
        assert phase() == "visible", phase()
        pg.evaluate("()=>window.__harness.emitAssistant('idle')"); pg.wait_for_timeout(1500)
        assert phase() == "hidden", phase()
    check("stays visible while the assistant is busy, hides when it is idle", assistant_busy_holds)

    def screenshot_reveals():
        fresh(); pg.wait_for_timeout(1200); assert phase() == "hidden"
        pg.evaluate("()=>window.__harness.addShot({w:640,h:400})"); pg.wait_for_timeout(600)
        assert phase() == "visible", phase()
        expect(pg.locator('[data-testid="toast-title"]')).to_be_visible()
    check("a screenshot confirmation reveals a hidden island", screenshot_reveals)

    def shortcut_reveals():
        fresh(); pg.wait_for_timeout(1200); assert phase() == "hidden"
        pg.evaluate("()=>window.__harness.emitIslandReveal()"); pg.wait_for_timeout(400)
        assert phase() == "visible", phase()
    check("the shortcut / tray Show VEX (island-reveal) reveals a hidden island", shortcut_reveals)

    def background_does_not_reveal():
        fresh(); pg.wait_for_timeout(1200); assert phase() == "hidden"
        n = len(hidden_calls())
        pg.evaluate("()=>window.__harness.notify({appId:'a',appName:'Chat',title:'Hi',body:'there'})"); pg.wait_for_timeout(600)
        assert phase() == "hidden", "a notification revealed the island"
        pg.evaluate("()=>window.__harness.emitAssistant('idle')"); pg.wait_for_timeout(300)
        assert phase() == "hidden" and all(c["hidden"] for c in hidden_calls()[n:])
    check("a notification does not reveal a hidden island", background_does_not_reveal)

    def switching_to_always_reveals():
        fresh(); pg.wait_for_timeout(1200); assert phase() == "hidden"
        pg.evaluate("()=>window.__harness.patchSettings({island:{visibility:'always'}})"); pg.wait_for_timeout(500)
        assert phase() == "visible", phase()
        pg.wait_for_timeout(1500); assert phase() == "visible"
    check("switching to Always Visible while hidden reveals it and it stays", switching_to_always_reveals)

    def native_refuses_in_always():
        fresh(auto=False)
        pg.wait_for_timeout(1500)
        assert hidden_calls() == []
    check("Always mode never asks the shell to hide", native_refuses_in_always)

    def settings_ui():
        fresh(auto=False)
        s = ctx.new_page(); s.goto(BASE + "/settings.html"); s.wait_for_selector('[data-testid="page-island"]')
        s.locator('[data-testid="page-island"]').click(); s.wait_for_timeout(300)
        assert s.locator('[data-testid="autohide-delay"]').count() == 0
        s.locator('[data-testid="visibility-auto"]').click(); s.wait_for_timeout(300)
        assert s.locator('[data-testid="autohide-delay"]').count() == 1
        v = json.loads(s.evaluate("localStorage.getItem('islet-test-settings')"))["island"]
        assert v["visibility"] == "auto", v
        s.locator('[data-testid="visibility-always"]').click(); s.wait_for_timeout(300)
        assert s.locator('[data-testid="autohide-delay"]').count() == 0
        s.close()
    check("Settings: visibility switch persists and the delay shows only in Auto-Hide", settings_ui)

    def volume_panel_holds():
        fresh(delay=300)
        isl.hover(); pg.wait_for_timeout(900)
        pg.locator('[data-testid="btn-volume"]').click(); pg.wait_for_timeout(300)
        pg.mouse.move(5, 400); pg.wait_for_timeout(250)
        assert phase() == "visible"
        # leaving collapses the island, which closes the panel; only then may it hide
        pg.wait_for_timeout(2500)
        assert pg.locator('[data-testid="volume-popover"]').count() == 0 and phase() == "hidden", phase()
        strip.hover(); pg.wait_for_timeout(500); assert phase() == "visible"
    check("integration: the volume panel keeps the island visible; after it collapses, Auto-Hide hides and the strip reveals", volume_panel_holds)

    def moved_island_strip():
        fresh(delay=300)
        pg.evaluate("()=>window.__harness.patchSettings({island:{offsetX:120}})"); pg.wait_for_timeout(300)
        pg.wait_for_timeout(1200); assert phase() == "hidden"
        strip.hover(); pg.wait_for_timeout(500); assert phase() == "visible"
        assert pg.evaluate("()=>window.__harness.hiddenCalls.at(-1).hidden") is False
    check("integration: a saved off-centre position does not break hide and reveal", moved_island_strip)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} auto-hide checks passed")
sys.exit(0 if passed == len(results) else 1)
