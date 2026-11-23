"""World Clock widget checks (run against `npm run dev`, test backend, fixed clock and time zone)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

def clock(y, mo, d, h, mi, s, ms=0):
    return """(()=>{const R=Date,start=R.now(),base=R.UTC(%d,%d,%d,%d,%d,%d,%d);const now=()=>base+(R.now()-start);
      function D(...a){if(!(this instanceof D))return new R(now()).toString();return a.length?new R(...a):new R(now());}
      D.prototype=R.prototype;D.now=now;D.UTC=R.UTC;D.parse=R.parse;window.Date=D;})()""" % (y, mo - 1, d, h, mi, s, ms)

LAYOUT = {"version": 1, "widgets": [{"kind": "worldclock", "size": "l", "pinned": False}]}

with sync_playwright() as p:
    b = p.chromium.launch()
    # 2026-11-23 12:00:05 UTC (every page load restarts the clock there); the viewer is in Kolkata (UTC+5:30)
    ctx = b.new_context(viewport={"width": 1100, "height": 760}, timezone_id="Asia/Kolkata", locale="en-GB")
    ctx.add_init_script(clock(2026, 11, 23, 12, 0, 5))
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    T = lambda t: pg.locator(f'[data-testid="{t}"]')
    row = lambda z: pg.locator(f'[data-testid="wc-row"][data-zone="{z}"]')

    def fresh(data=None, size="l", page=None):
        q = page or pg
        q.evaluate("()=>window.__harness.hub.reset()")
        q.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "worldclock", "size": size, "pinned": False}]})
        if data is not None: q.evaluate("d=>localStorage.setItem('islet-test-hub-data:worldclock', JSON.stringify(d))", data)
        q.reload(); q.wait_for_selector('[data-testid="widget-worldclock"][data-view]'); q.wait_for_timeout(300)
    def zones(q=None): return (q or pg).locator('[data-testid="wc-row"]').evaluate_all("els=>els.map(e=>e.dataset.zone)")
    def stored(): return json.loads(pg.evaluate("localStorage.getItem('islet-test-hub-data:worldclock')") or "null")

    def starter():
        fresh()
        assert zones() == ["local", "Europe/London", "America/New_York", "Asia/Tokyo"], zones()
        expect(row("local").locator('[data-testid="wc-time"]')).to_have_text("17:30")
        expect(row("Europe/London").locator('[data-testid="wc-time"]')).to_have_text("12:00")
        expect(row("Europe/London").locator('[data-testid="wc-sub"]')).to_have_text("Today · −5h 30m")
        expect(row("America/New_York").locator('[data-testid="wc-time"]')).to_have_text("07:00")
        expect(row("America/New_York").locator('[data-testid="wc-sub"]')).to_have_text("Today · −10h 30m")
        expect(row("Asia/Tokyo").locator('[data-testid="wc-time"]')).to_have_text("21:00")
        expect(row("Asia/Tokyo").locator('[data-testid="wc-sub"]')).to_have_text("Today · +3h 30m")
        expect(row("local").locator('[data-testid="wc-sub"]')).to_have_text("Your time zone")
    check("a fresh card shows your own time plus London, New York and Tokyo with correct times and differences", starter)

    def minute_tick():
        ctx3 = b.new_context(viewport={"width": 1100, "height": 760}, timezone_id="Asia/Kolkata", locale="en-GB")
        ctx3.add_init_script(clock(2026, 11, 23, 12, 0, 58, 500))
        q = ctx3.new_page(); q.goto(BASE + "/hub.html"); q.wait_for_selector('[data-testid="hub"]')
        fresh(page=q)
        pg_ = q
        expect(pg_.locator('[data-testid="wc-row"][data-zone="local"] [data-testid="wc-time"]')).to_have_text("17:30")
        pg_.wait_for_timeout(1900)
        expect(pg_.locator('[data-testid="wc-row"][data-zone="local"] [data-testid="wc-time"]')).to_have_text("17:31")
        expect(pg_.locator('[data-testid="wc-row"][data-zone="Asia/Tokyo"] [data-testid="wc-time"]')).to_have_text("21:01")
        ctx3.close()
    check("the clocks move on at the next whole minute without a reload", minute_tick)

    def day_night():
        fresh()
        assert row("Europe/London").get_attribute("data-day") == "day"
        assert row("America/New_York").get_attribute("data-day") == "day"
        assert row("Asia/Tokyo").get_attribute("data-day") == "night"
    check("day and night follow each city's own hour", day_night)

    def format_cycle():
        fresh(); T("wc-format").click(); pg.wait_for_timeout(150)
        expect(T("wc-format")).to_have_text("24 h"); expect(row("local").locator('[data-testid="wc-time"]')).to_have_text("17:30")
        T("wc-format").click(); pg.wait_for_timeout(150)
        expect(T("wc-format")).to_have_text("12 h"); expect(row("local").locator('[data-testid="wc-time"]')).to_have_text(__import__("re").compile(r"^5:30\s?pm$", 2))
        pg.wait_for_timeout(600); assert stored()["hour12"] is True
        pg.reload(); pg.wait_for_selector('[data-testid="widget-worldclock"][data-view]'); pg.wait_for_timeout(300)
        expect(T("wc-format")).to_have_text("12 h")
        T("wc-format").click(); expect(T("wc-format")).to_have_text("System")
    check("the 12/24 hour switch cycles System, 24 h, 12 h and is remembered", format_cycle)

    def add_city():
        fresh(); T("wc-add").click(); T("wc-search").fill("sing"); pg.wait_for_timeout(150)
        assert T("wc-result").count() == 1 and T("wc-result").first.get_attribute("data-zone") == "Asia/Singapore"
        T("wc-result").first.click(); pg.wait_for_timeout(200)
        assert zones()[-1] == "Asia/Singapore"
        expect(row("Asia/Singapore").locator('[data-testid="wc-time"]')).to_have_text("20:00")
        expect(row("Asia/Singapore").locator('[data-testid="wc-sub"]')).to_have_text("Today · +2h 30m")
        T("wc-search").fill("sing"); pg.wait_for_timeout(150); expect(T("wc-none")).to_be_visible()  # already added: not offered again
        T("wc-search").fill("zzzz"); pg.wait_for_timeout(150); expect(T("wc-none")).to_have_text("No city matches that.")
        T("wc-add").click()
        pg.wait_for_timeout(600); pg.reload(); pg.wait_for_selector('[data-testid="widget-worldclock"][data-view]'); pg.wait_for_timeout(300)
        assert zones()[-1] == "Asia/Singapore"
    check("a city can be searched, added once, and is there after a reload", add_city)

    def next_day_label():
        fresh({"version": 1, "hour12": None, "clocks": [{"zone": "Pacific/Auckland", "label": "Auckland"}, {"zone": "Pacific/Pago_Pago", "label": "Pago Pago"}]})
        expect(row("Pacific/Auckland").locator('[data-testid="wc-sub"]')).to_have_text("Tomorrow · +7h 30m")
        expect(row("Pacific/Pago_Pago").locator('[data-testid="wc-sub"]')).to_have_text("Today · −16h 30m")
    check("a city already on the next calendar day says Tomorrow", next_day_label)

    def edit_remove_reorder():
        fresh(); T("wc-edit").click()
        assert T("wc-remove").count() == 3 and row("local").locator('[data-testid="wc-remove"]').count() == 0
        expect(row("Europe/London").locator('[data-testid="wc-up"]')).to_be_disabled()
        expect(row("Asia/Tokyo").locator('[data-testid="wc-down"]')).to_be_disabled()
        row("Asia/Tokyo").locator('[data-testid="wc-up"]').click(); pg.wait_for_timeout(150)
        assert zones() == ["local", "Europe/London", "Asia/Tokyo", "America/New_York"], zones()
        row("Europe/London").locator('[data-testid="wc-remove"]').click(); pg.wait_for_timeout(150)
        assert zones() == ["local", "Asia/Tokyo", "America/New_York"]
        T("wc-edit").click(); assert T("wc-remove").count() == 0
        pg.wait_for_timeout(600); assert [c["zone"] for c in stored()["clocks"]] == ["Asia/Tokyo", "America/New_York"]
    check("Edit mode reorders and removes clocks (never the local row) and saves", edit_remove_reorder)

    def remove_all():
        fresh(); T("wc-edit").click()
        for _ in range(3): T("wc-remove").first.click(); pg.wait_for_timeout(100)
        expect(T("wc-empty")).to_be_visible(); pg.wait_for_timeout(600)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-worldclock"][data-view]'); pg.wait_for_timeout(300)
        expect(T("wc-empty")).to_be_visible(); assert zones() == ["local"]
    check("an empty list the user chose stays empty (the starter clocks do not come back)", remove_all)

    def cap():
        fresh({"version": 1, "hour12": None, "clocks": [{"zone": z, "label": z} for z in [
            "Asia/Tokyo", "Asia/Seoul", "Asia/Dubai", "Europe/Paris", "Europe/Rome", "Europe/Berlin", "America/Chicago", "America/Denver", "America/Lima", "Africa/Cairo", "Asia/Manila"]]})
        T("wc-add").click(); T("wc-search").fill("sydney"); T("wc-result").first.click(); pg.wait_for_timeout(200)
        assert len(zones()) == 13  # local + 12
        T("wc-add").click(); expect(T("wc-add")).to_be_disabled()
    check("up to 12 clocks, then Add city is disabled", cap)

    def corrupt():
        fresh({"version": 1, "hour12": "x", "clocks": [{"zone": "Asia/Tokyo"}, {"zone": "Asia/Tokyo"}, {"zone": "Mars/Base"}, 7, None, {"zone": "UTC", "label": ""}]})
        assert zones() == ["local", "Asia/Tokyo", "UTC"], zones()
        expect(row("UTC").locator('[data-testid="wc-label"]')).to_have_text("UTC")
    check("a damaged saved list is repaired", corrupt)

    def sizes():
        for size in ("m", "l"):
            fresh(size=size)
            for t in ("wc-add", "wc-format", "wc-edit"): expect(T(t)).to_be_visible()
            bb = row("local").bounding_box(); cb = pg.locator('[data-testid="widget-card"]').bounding_box()
            assert bb["y"] + bb["height"] <= cb["y"] + cb["height"] + 1, (size, bb, cb)
    check("both card sizes show the controls and the local row inside the card", sizes)

    def gallery():
        pg.evaluate("()=>window.__harness.hub.reset()"); pg.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": []})
        pg.reload(); pg.wait_for_selector('[data-testid="hub"]'); pg.wait_for_timeout(300)
        T("hub-add").click(); expect(pg.locator('[data-testid="gallery-item"][data-kind="worldclock"]')).to_be_visible()
    check("the gallery offers World clock", gallery)

    # ---- daylight saving: the same card on 2026-07-15 (northern summer time)
    ctx2 = b.new_context(viewport={"width": 1100, "height": 760}, timezone_id="Asia/Kolkata", locale="en-GB")
    ctx2.add_init_script(clock(2026, 7, 15, 12, 0, 10))
    pg2 = ctx2.new_page(); pg2.on("pageerror", lambda e: errors.append(str(e)))
    pg2.goto(BASE + "/hub.html"); pg2.wait_for_selector('[data-testid="hub"]')
    def dst():
        fresh(page=pg2)
        r = lambda z, t: pg2.locator(f'[data-testid="wc-row"][data-zone="{z}"] [data-testid="{t}"]')
        expect(r("Europe/London", "wc-time")).to_have_text("13:00"); expect(r("Europe/London", "wc-sub")).to_have_text("Today · −4h 30m")
        expect(r("America/New_York", "wc-time")).to_have_text("08:00"); expect(r("America/New_York", "wc-sub")).to_have_text("Today · −9h 30m")
    check("daylight saving is applied: in July London is UTC+1 and New York UTC-4", dst)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} world clock checks passed")
sys.exit(0 if passed == len(results) else 1)
