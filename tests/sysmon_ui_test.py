"""System Monitor widget checks (run against `npm run dev`, simulated native readings)."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

with sync_playwright() as p:
    b = p.chromium.launch()
    pg = b.new_context(viewport={"width": 1100, "height": 760}).new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    T = lambda t: pg.locator(f'[data-testid="{t}"]')
    H = lambda js, *a: pg.evaluate(js, *a)

    def fresh():
        H("()=>window.__harness.hub.reset()")
        H("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "sysmon", "size": "m", "pinned": False}]})
        pg.reload(); pg.wait_for_selector('[data-testid="widget-sysmon"][data-view="ready"]')

    def shows():
        fresh()
        expect(T("sm-cpu-value")).to_have_text("23%")
        expect(T("sm-mem-value")).to_have_text("61%")
        expect(T("sm-mem-sub")).to_have_text("9.8 GB of 16.0 GB")
        expect(T("sm-down")).to_have_text("↓ 1.2 MB/s")
        expect(T("sm-up")).to_have_text("↑ 340 KB/s")
        expect(T("sm-battery-value")).to_have_text("80%")
        expect(T("sm-battery-sub")).to_have_text("On battery · 3 h 10 min left")
    check("shows CPU, memory, network and battery from the readings", shows)

    def updates():
        fresh()
        H("()=>window.__harness.sysmon.set({cpuPercent: 95, downBps: 0})")
        pg.wait_for_timeout(2600)
        expect(T("sm-cpu-value")).to_have_text("95%")
        expect(T("sm-cpu")).to_have_attribute("data-level", "high")
        expect(T("sm-down")).to_have_text("↓ 0 B/s")
    check("refreshes on its own and marks a high load", updates)

    def levels():
        fresh()
        H("()=>window.__harness.sysmon.set({battery:{percent:10,charging:false,plugged:false,minutesLeft:null}, cpuPercent:null})")
        pg.wait_for_timeout(2600)
        expect(T("sm-battery")).to_have_attribute("data-level", "high")
        expect(T("sm-cpu-value")).to_have_text("…")
        H("()=>window.__harness.sysmon.set({battery:null})")
        pg.wait_for_timeout(2600)
        expect(T("sm-nobattery")).to_be_visible()
    check("a low battery is flagged, a missing CPU rate shows a placeholder, and no battery is stated", levels)

    def interval():
        fresh()
        T("sm-int-5").click()
        expect(T("sm-int-5")).to_have_attribute("aria-pressed", "true")
        n = H("()=>window.__harness.sysmon.samples")
        pg.wait_for_timeout(3600)
        assert H("()=>window.__harness.sysmon.samples") - n <= 1, "5 s interval should sample at most once in 3.6 s"
        pg.wait_for_timeout(900)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-sysmon"][data-view="ready"]')
        expect(T("sm-int-5")).to_have_attribute("aria-pressed", "true")
    check("the refresh interval can be changed and is remembered", interval)

    def stops_when_hidden():
        fresh()
        # leave the card (switch layout to empty): polling must stop
        H("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": []})
        pg.wait_for_timeout(600)
        n = H("()=>window.__harness.sysmon.samples")
        pg.wait_for_timeout(4500)
        assert H("()=>window.__harness.sysmon.samples") == n, "still polling after the card was removed"
    check("polling stops when the card is gone", stops_when_hidden)

    def failure():
        fresh()
        H("()=>window.__harness.sysmon.failNext('Windows said no')")
        pg.wait_for_selector('[data-testid="sm-error"]', timeout=6000)
        expect(T("sm-error")).to_contain_text("Could not refresh: Windows said no")
        expect(T("sm-cpu-value")).to_be_visible()  # the last good numbers stay
        pg.wait_for_function("()=>!document.querySelector('[data-testid=sm-error]')", timeout=6000)
    check("a failed reading shows an error and recovers on the next one", failure)

    def unavailable():
        fresh()
        H("()=>window.__harness.sysmon.set({available:false,error:'System Monitor is available on Windows only'})")
        pg.wait_for_selector('[data-testid="sm-error"]', timeout=6000)
        expect(T("sm-error")).to_contain_text("Windows only")
        H("()=>window.__harness.sysmon.set({available:true,error:null})")
    check("an unavailable monitor says so instead of showing zeros", unavailable)

    check("no console errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
