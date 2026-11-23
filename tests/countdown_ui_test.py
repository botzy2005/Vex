"""Countdown widget checks (run against `npm run dev`, test backend)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 1100, "height": 760})
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    H = lambda js: pg.evaluate("()=>window.__harness.hub." + js)
    T = lambda t: pg.locator(f'[data-testid="{t}"]')
    rows = lambda: pg.locator('[data-testid="cd-row"]')

    def fresh(size="l"):
        pg.evaluate("()=>window.__harness.hub.reset()")
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "countdown", "size": size, "pinned": False}]})
        pg.reload(); pg.wait_for_selector('[data-testid="widget-countdown"]'); pg.wait_for_timeout(400)
    def mine(): return [t for t in H("timers()") if t["owner"] == "countdown"]
    def isl_status(): return [s for s in H("statuses()") if s["kind"] == "countdown"]

    def empty():
        fresh(); expect(T("cd-empty")).to_be_visible(); assert T("cd-preset").count() == 6 and not mine()
    check("a fresh card offers six quick countdowns and starts none", empty)

    def preset_starts():
        fresh(); T("cd-preset").nth(1).click(); pg.wait_for_timeout(1400)
        assert rows().count() == 1 and rows().first.get_attribute("data-state") == "running"
        expect(T("cd-name")).to_have_text("5 min"); assert T("cd-time").inner_text() in ("4:59", "4:58"), T("cd-time").inner_text()
        assert mine()[0]["durationMs"] == 300000
    check("a quick countdown is created and starts at once", preset_starts)

    def custom():
        fresh(); T("cd-label").fill("Tea"); T("cd-m").fill("3"); T("cd-s").fill("30"); T("cd-start").click(); pg.wait_for_timeout(300)
        expect(T("cd-name")).to_have_text("Tea"); assert mine()[0]["durationMs"] == 210000
        assert T("cd-label").input_value() == "" and T("cd-m").input_value() == ""
        T("cd-h").fill("1"); T("cd-m").fill("90"); T("cd-h").press("Enter"); pg.wait_for_timeout(300)
        assert [t["durationMs"] for t in mine()] == [210000, 9000000]
    check("a custom time and label work, the form clears, and Enter submits", custom)

    def bad_input():
        fresh()
        for h, m, s, msg in [("", "", "", "at least one second"), ("x", "", "", "whole numbers"), ("100", "", "", "99 hours")]:
            T("cd-h").fill(h); T("cd-m").fill(m); T("cd-s").fill(s); T("cd-start").click(); pg.wait_for_timeout(100)
            expect(T("cd-problem")).to_contain_text(msg)
        assert not mine()
    check("bad times are refused with a reason and create nothing", bad_input)

    def controls():
        fresh(); T("cd-preset").first.click(); pg.wait_for_timeout(1300)
        T("cd-pause").click(); pg.wait_for_timeout(200); assert rows().first.get_attribute("data-state") == "paused"
        frozen = T("cd-time").inner_text(); pg.wait_for_timeout(1300); assert T("cd-time").inner_text() == frozen
        T("cd-resume").click(); pg.wait_for_timeout(200); assert rows().first.get_attribute("data-state") == "running"
        T("cd-reset").click(); pg.wait_for_timeout(200)
        assert rows().first.get_attribute("data-state") == "idle"; expect(T("cd-time")).to_have_text("1:00")
        T("cd-resume").click(); pg.wait_for_timeout(200); assert rows().first.get_attribute("data-state") == "running"
        T("cd-delete").click(); pg.wait_for_timeout(200); assert rows().count() == 0 and not mine()
    check("pause freezes, resume continues, reset restores, delete removes", controls)

    def finish():
        fresh(); T("cd-preset").first.click(); pg.wait_for_timeout(300); H("expire()"); pg.wait_for_timeout(500)
        assert rows().first.get_attribute("data-state") == "finished"; expect(T("cd-time")).to_have_text("0:00")
        expect(rows().first.locator("small")).to_have_text("Time's up")
        T("cd-restart").click(); pg.wait_for_timeout(300); assert rows().first.get_attribute("data-state") == "running"
    check("a finished countdown says Time's up and can be restarted", finish)

    def several():
        fresh(); T("cd-preset").nth(0).click(); T("cd-preset").nth(2).click(); pg.wait_for_timeout(500)
        assert rows().count() == 2 and all(r == "running" for r in rows().evaluate_all("e=>e.map(x=>x.dataset.state)"))
        st = isl_status(); assert len(st) == 1 and st[0]["label"] == "1 min" and st[0]["detail"].startswith("ends "), st
        T("cd-delete").first.click(); pg.wait_for_timeout(400); assert isl_status()[0]["label"] == "10 min"
        T("cd-delete").first.click(); pg.wait_for_timeout(400); assert not isl_status()
    check("several run at once and the island shows the one that ends first", several)

    def cap():
        fresh()
        for _ in range(8): T("cd-preset").first.click(); pg.wait_for_timeout(80)
        pg.wait_for_timeout(300); assert rows().count() == 8
        assert T("cd-preset").first.is_disabled() and T("cd-start").is_disabled()
    check("at most 8 countdowns", cap)

    def survives_reload_and_close():
        fresh(); T("cd-preset").nth(2).click(); pg.wait_for_timeout(400)
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]'); pg.wait_for_timeout(300)
        pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="widget-countdown"]'); pg.wait_for_timeout(1200)
        assert rows().count() == 1 and rows().first.get_attribute("data-state") == "running" and len(mine()) == 1
    check("a running countdown keeps running while the Hub is closed", survives_reload_and_close)

    def island_chime():
        fresh()
        isl = ctx.new_page(); isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]'); isl.wait_for_timeout(400)
        chimes = lambda: isl.evaluate("()=>window.__harness.chimeCount||0")
        T("cd-preset").first.click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(800); assert chimes() == 1, chimes()
        T("cd-sound").uncheck(); pg.wait_for_timeout(700)
        T("cd-restart").click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(800); assert chimes() == 1, chimes()
        T("cd-sound").check(); pg.wait_for_timeout(700)
        T("cd-restart").click(); pg.wait_for_timeout(300); H("expire(undefined, 300000)"); isl.wait_for_timeout(800); assert chimes() == 1, chimes()
        isl.close()
    check("the island chimes when a countdown ends, honours the setting, and ignores stale ends", island_chime)

    def never_reveals():
        fresh()
        isl = ctx.new_page(); isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]'); isl.wait_for_timeout(400)
        isl.evaluate("()=>window.__harness.patchSettings({island:{visibility:'auto',autoHideDelayMs:300}})"); isl.mouse.move(5, 5); isl.wait_for_timeout(1500)
        assert isl.locator(".stage").get_attribute("data-hide-phase") == "hidden"
        T("cd-preset").first.click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(1200)
        assert isl.locator(".stage").get_attribute("data-hide-phase") == "hidden", "a countdown ending must not reveal the island"
        isl.close()
    check("a countdown ending never reveals a hidden island", never_reveals)

    def sizes():
        for size in ("m", "l"):
            fresh(size); T("cd-preset").first.click(); pg.wait_for_timeout(300)
            for t in ("cd-start", "cd-time", "cd-sound"): expect(T(t)).to_be_visible()
    check("both card sizes keep the controls visible", sizes)

    def gallery():
        pg.evaluate("()=>window.__harness.hub.reset()"); pg.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": []})
        pg.reload(); pg.wait_for_selector('[data-testid="hub"]'); pg.wait_for_timeout(300)
        T("hub-add").click(); expect(pg.locator('[data-testid="gallery-item"][data-kind="countdown"]')).to_be_visible()
    check("the gallery offers Countdown", gallery)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} countdown checks passed")
sys.exit(0 if passed == len(results) else 1)
