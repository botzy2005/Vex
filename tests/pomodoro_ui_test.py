"""Pomodoro widget checks (run against `npm run dev`, test backend)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

LAYOUT = {"version": 1, "widgets": [{"kind": "pomodoro", "size": "l", "pinned": False}]}

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

    def fresh(layout=LAYOUT, data=None, wait=True):
        pg.evaluate("()=>window.__harness.hub.reset()")
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", layout)
        if data is not None:
            pg.evaluate("d=>localStorage.setItem('islet-test-hub-data:pomodoro', JSON.stringify(d))", data)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-pomodoro"]' if wait else '[data-testid="hub"]'); pg.wait_for_timeout(500)
    def mine(): return [t for t in H("timers()") if t["owner"] == "pomodoro"]
    def stored(): return json.loads(pg.evaluate("localStorage.getItem('islet-test-hub-data:pomodoro')") or "{}")
    def state(): return pg.locator('[data-testid="widget-pomodoro"]').get_attribute("data-state")
    def time(): return T("pomo-time").inner_text()

    def first_run():
        fresh()
        expect(T("pomo-phase")).to_have_text("Focus"); expect(T("pomo-time")).to_have_text("25:00")
        expect(T("pomo-today")).to_have_text("Today: 0 focus sessions")
        assert len(mine()) == 1 and mine()[0]["durationMs"] == 25 * 60000 and mine()[0]["state"] == "idle", mine()
        assert pg.locator('[data-testid="pomo-dots"] i').count() == 4
    check("a fresh Pomodoro shows 25:00 Focus, 4 session dots and creates exactly one timer", first_run)

    def start_pause_resume_reset():
        fresh()
        T("pomo-start").click(); pg.wait_for_timeout(1300)
        assert state() == "running" and time() != "25:00", time()
        expect(T("pomo-start")).to_have_text("Pause")
        T("pomo-start").click(); pg.wait_for_timeout(300)
        assert state() == "paused"; expect(T("pomo-phase")).to_contain_text("paused"); expect(T("pomo-start")).to_have_text("Resume")
        frozen = time(); pg.wait_for_timeout(1300); assert time() == frozen
        T("pomo-start").click(); pg.wait_for_timeout(300); assert state() == "running"
        T("pomo-reset").click(); pg.wait_for_timeout(300)
        assert state() == "idle"; expect(T("pomo-time")).to_have_text("25:00")
    check("start, pause (time freezes), resume and reset", start_pause_resume_reset)

    def status_for_island():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(500)
        st = [s for s in H("statuses()") if s["kind"] == "pomodoro"]
        assert len(st) == 1 and st[0]["label"] == "Focus" and st[0]["detail"].startswith("ends "), st
        T("pomo-start").click(); pg.wait_for_timeout(300)
        assert [s for s in H("statuses()") if s["kind"] == "pomodoro"][0]["detail"] == "paused"
        T("pomo-reset").click(); pg.wait_for_timeout(300)
        assert not [s for s in H("statuses()") if s["kind"] == "pomodoro"]
    check("the island gets a status while running or paused, and none when idle", status_for_island)

    def finish_focus_counts_and_moves_on():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(300)
        H("expire()"); pg.wait_for_timeout(700)
        expect(T("pomo-phase")).to_have_text("Short break"); expect(T("pomo-time")).to_have_text("5:00")
        expect(T("pomo-today")).to_have_text("Today: 1 focus session"); expect(T("pomo-notice")).to_have_text("Focus finished")
        assert state() == "idle", "the next phase must wait for Start by default"
        assert len(mine()) == 1, mine()
        assert pg.locator('[data-testid="pomo-dots"] i[data-on="true"]').count() == 1
        assert stored()["cycle"] == 1
    check("finishing a focus session counts it and offers the short break without auto-starting", finish_focus_counts_and_moves_on)

    def break_does_not_count():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); pg.wait_for_timeout(600)
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); pg.wait_for_timeout(600)
        expect(T("pomo-phase")).to_have_text("Focus"); expect(T("pomo-today")).to_have_text("Today: 1 focus session")
    check("finishing a break does not count and returns to Focus", break_does_not_count)

    def long_break_after_four():
        fresh()
        for i in range(4):
            T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
            if i < 3:
                expect(T("pomo-phase")).to_have_text("Short break"); T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
        expect(T("pomo-phase")).to_have_text("Long break"); expect(T("pomo-time")).to_have_text("15:00")
        expect(T("pomo-today")).to_have_text("Today: 4 focus sessions")
        assert pg.locator('[data-testid="pomo-dots"] i[data-on="true"]').count() == 4
        T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
        expect(T("pomo-phase")).to_have_text("Focus")
        assert pg.locator('[data-testid="pomo-dots"] i[data-on="true"]').count() == 0
    check("a long break comes after the fourth focus session and a new set starts after it", long_break_after_four)

    def skip():
        fresh(); T("pomo-skip").click(); pg.wait_for_timeout(500)
        expect(T("pomo-phase")).to_have_text("Short break"); expect(T("pomo-today")).to_have_text("Today: 0 focus sessions")
        assert len(mine()) == 1 and stored()["cycle"] == 0
        T("pomo-skip").click(); pg.wait_for_timeout(500); expect(T("pomo-phase")).to_have_text("Focus")
    check("Skip moves to the next phase without counting anything", skip)

    def autostart_live():
        fresh(); T("pomo-gear").click(); T("pomo-autostart").check(); pg.wait_for_timeout(300); T("pomo-gear").click()
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); pg.wait_for_timeout(900)
        expect(T("pomo-phase")).to_have_text("Short break"); assert state() == "running", state()
    check("auto-start starts the next phase when a phase ends live", autostart_live)

    def autostart_not_when_late():
        fresh(); T("pomo-gear").click(); T("pomo-autostart").check(); pg.wait_for_timeout(300); T("pomo-gear").click()
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire(undefined, 120000)"); pg.wait_for_timeout(900)
        expect(T("pomo-phase")).to_have_text("Short break"); assert state() == "idle", state()
    check("auto-start never fires for a phase that ended long ago (Hub closed / PC asleep)", autostart_not_when_late)

    def ended_while_hub_closed():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(400)
        # the Hub window closes, the native timer keeps running and ends
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]'); pg.wait_for_timeout(300)
        H("expire(undefined, 600000)"); pg.wait_for_timeout(300)
        assert [t["state"] for t in mine()] == ["finished"]
        pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="widget-pomodoro"]'); pg.wait_for_timeout(900)
        expect(T("pomo-phase")).to_have_text("Short break"); expect(T("pomo-today")).to_have_text("Today: 1 focus session")
        assert state() == "idle" and len(mine()) == 1, (state(), mine())
        pg.reload(); pg.wait_for_selector('[data-testid="widget-pomodoro"]'); pg.wait_for_timeout(700)
        expect(T("pomo-today")).to_have_text("Today: 1 focus session")
    check("a phase that ended while the Hub was closed is counted once when it opens", ended_while_hub_closed)

    def survives_reload_running():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(400)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-pomodoro"]'); pg.wait_for_timeout(2200)
        assert state() == "running" and len(mine()) == 1, (state(), mine())
        t = time(); assert t != "25:00", t
    check("a running phase keeps running across a reload with the same timer", survives_reload_running)

    def settings_apply():
        fresh(); T("pomo-gear").click()
        T("pomo-focus-min").fill("30"); T("pomo-focus-min").press("Enter"); pg.wait_for_timeout(300)
        T("pomo-short-min").fill("7"); T("pomo-short-min").press("Enter")
        T("pomo-long-every").fill("2"); T("pomo-long-every").press("Enter"); pg.wait_for_timeout(500)
        T("pomo-gear").click(); expect(T("pomo-time")).to_have_text("30:00")
        assert pg.locator('[data-testid="pomo-dots"] i').count() == 2
        T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
        T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
        T("pomo-start").click(); pg.wait_for_timeout(250); H("expire()"); pg.wait_for_timeout(500)
        expect(T("pomo-phase")).to_have_text("Long break")
        s = stored()["settings"]; assert (s["focusMin"], s["shortMin"], s["longEvery"]) == (30, 7, 2), s
    check("custom lengths and long-break interval apply and persist", settings_apply)

    def settings_clamp_and_running():
        fresh(); T("pomo-gear").click()
        T("pomo-focus-min").fill("9999"); T("pomo-focus-min").press("Enter"); pg.wait_for_timeout(700)
        assert stored()["settings"]["focusMin"] == 180
        T("pomo-focus-min").fill("0"); T("pomo-focus-min").press("Enter"); pg.wait_for_timeout(700)
        assert stored()["settings"]["focusMin"] == 1
        T("pomo-focus-min").fill("25"); T("pomo-focus-min").press("Enter"); pg.wait_for_timeout(300)
        T("pomo-gear").click(); T("pomo-start").click(); pg.wait_for_timeout(400)
        T("pomo-gear").click(); T("pomo-focus-min").fill("50"); T("pomo-focus-min").press("Enter"); pg.wait_for_timeout(400)
        assert mine()[0]["durationMs"] == 25 * 60000, "a running phase must keep its length"
    check("lengths are clamped to 1-180 and a running phase keeps its own length", settings_clamp_and_running)

    def corrupt_data():
        fresh(data={"version": 1, "settings": {"focusMin": "x"}, "phase": "bogus", "cycle": 77, "timerId": "no", "history": 5})
        expect(T("pomo-phase")).to_have_text("Focus"); expect(T("pomo-time")).to_have_text("25:00")
        assert len(mine()) == 1
    check("a damaged saved document is repaired, not a crash", corrupt_data)

    def orphan_timers():
        fresh()
        pg.evaluate("""()=>{const h=window.__harness.hub.api;return h.timerCreate('pomodoro','Stray',60000).then(()=>h.timerCreate('pomodoro','Stray 2',60000))}""")
        pg.evaluate("()=>localStorage.setItem('islet-test-hub-data:pomodoro', JSON.stringify({version:1,timerId:null}))")
        pg.reload(); pg.wait_for_selector('[data-testid="widget-pomodoro"]'); pg.wait_for_timeout(800)
        assert len(mine()) == 1, [t['label'] for t in mine()]
    check("stray Pomodoro timers do not multiply", orphan_timers)

    def removing_keeps_progress():
        fresh(); T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); pg.wait_for_timeout(600)
        pg.evaluate("()=>window.__harness.hub.setLayout({version:1,widgets:[]})"); pg.wait_for_timeout(400)
        assert pg.locator('[data-testid="widget-pomodoro"]').count() == 0
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", LAYOUT); pg.wait_for_timeout(600)
        expect(T("pomo-today")).to_have_text("Today: 1 focus session")
    check("removing and re-adding the card keeps today's count", removing_keeps_progress)

    def island_chime():
        fresh()
        isl = ctx.new_page(); isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]'); isl.wait_for_timeout(400)
        def chimes(): return isl.evaluate("()=>window.__harness.chimeCount||0")
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(800)
        assert chimes() == 1, chimes()
        # sound off: silent
        T("pomo-gear").click(); T("pomo-sound").uncheck(); pg.wait_for_timeout(700); T("pomo-gear").click()
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(800)
        assert chimes() == 1, chimes()
        # a stale end (VEX was closed) is silent even with sound on
        T("pomo-gear").click(); T("pomo-sound").check(); pg.wait_for_timeout(700); T("pomo-gear").click()
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire(undefined, 300000)"); isl.wait_for_timeout(800)
        assert chimes() == 1, chimes()
        isl.close()
    check("the island chimes when a phase ends, honours the sound setting, and stays silent for a stale end", island_chime)

    def never_reveals_island():
        fresh()
        isl = ctx.new_page(); isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]'); isl.wait_for_timeout(400)
        isl.evaluate("()=>window.__harness.patchSettings({island:{visibility:'auto',autoHideDelayMs:300}})"); isl.mouse.move(5, 5); isl.wait_for_timeout(1500)
        assert isl.locator(".stage").get_attribute("data-hide-phase") == "hidden"
        T("pomo-start").click(); pg.wait_for_timeout(300); H("expire()"); isl.wait_for_timeout(1200)
        assert isl.locator(".stage").get_attribute("data-hide-phase") == "hidden", "a Pomodoro end must not reveal the island"
        isl.close()
    check("a phase ending never reveals a hidden island", never_reveals_island)

    def sizes():
        for size in ("m", "l"):
            fresh({"version": 1, "widgets": [{"kind": "pomodoro", "size": size, "pinned": False}]})
            for t in ("pomo-time", "pomo-start", "pomo-skip", "pomo-today"):
                expect(T(t)).to_be_visible()
            bb = T("pomo-start").bounding_box(); cb = pg.locator('[data-testid="widget-card"]').bounding_box()
            assert bb["y"] + bb["height"] <= cb["y"] + cb["height"] + 1, (size, bb, cb)
    check("both card sizes show the controls inside the card", sizes)

    def gallery_lists_it():
        fresh({"version": 1, "widgets": []}, wait=False)
        pg.locator('[data-testid="hub-add"]').click(); pg.wait_for_timeout(300)
        item = pg.locator('[data-testid="gallery-item"][data-kind="pomodoro"]'); expect(item).to_be_visible()
    check("the gallery offers Pomodoro", gallery_lists_it)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} pomodoro checks passed")
sys.exit(0 if passed == len(results) else 1)
