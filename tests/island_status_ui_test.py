"""Collapsed-island status line checks (run against `npm run dev`): a running countdown on the Hub page shows on the island page."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 1100, "height": 760})
    hub = ctx.new_page(); isl = ctx.new_page()
    errors = []
    for q in (hub, isl):
        q.on("pageerror", lambda e: errors.append(str(e)))
        q.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    hub.goto(BASE + "/hub.html"); hub.wait_for_selector('[data-testid="hub"]')
    hub.evaluate("()=>window.__harness.hub.reset()")
    hub.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "countdown", "size": "m", "pinned": False}]})
    hub.reload(); hub.wait_for_selector('[data-testid="widget-countdown"]')
    isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]')
    I = lambda t: isl.locator(f'[data-testid="{t}"]')
    start = lambda: hub.locator('[data-testid="cd-preset"][data-ms="60000"]').click()

    def idle_first():
        expect(I("view-idle")).to_be_visible()
        assert I("view-status").count() == 0
    check("with no timer the island stays plain idle", idle_first)

    def shows():
        start()
        expect(I("view-status")).to_be_visible(timeout=4000)
        expect(I("status-text")).to_contain_text("1 min")
        expect(I("status-text")).to_contain_text("ends")
    check("a running countdown shows as one line on the idle island", shows)

    def media_dot():
        isl.evaluate("()=>window.__harness.setSession({title:'Song',artist:'A',playing:true,canPause:true})")
        expect(I("view-media")).to_be_visible(timeout=3000)
        expect(I("status-dot")).to_be_visible()
        assert I("view-status").count() == 0
        isl.evaluate("()=>window.__harness.setSession(null)")
        expect(I("view-status")).to_be_visible(timeout=3000)
    check("with music playing only a small dot is shown, and the line returns when it stops", media_dot)

    def setting_off():
        isl.evaluate("()=>window.__harness.patchSettings({island:{showWidgetStatus:false}})")
        expect(I("view-status")).to_have_count(0, timeout=3000)
        isl.evaluate("()=>window.__harness.patchSettings({island:{showWidgetStatus:true}})")
    check("turning the setting off removes the line; on brings it back", lambda: (setting_off(), expect(I("view-status")).to_be_visible(timeout=3000)))

    def clears():
        hub.locator('[data-testid="cd-delete"]').first.click()
        expect(I("view-status")).to_have_count(0, timeout=4000)
        expect(I("view-idle")).to_be_visible()
    check("deleting the countdown returns the island to idle", clears)

    check("no console errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
