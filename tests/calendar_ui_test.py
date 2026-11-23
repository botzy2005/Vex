"""Calendar widget checks (run against `npm run dev`, test backend, fixed clock 2026-11-23 17:30 Asia/Kolkata)."""
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

KEY = "islet-test-hub-data:calendar"

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 1100, "height": 760}, timezone_id="Asia/Kolkata", locale="en-GB")
    ctx.add_init_script(clock(2026, 11, 23, 12, 0, 5))
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    T = lambda t: pg.locator(f'[data-testid="{t}"]')
    day = lambda d: pg.locator(f'[data-testid="cal-day"][data-date="{d}"]')

    def fresh(doc=None):
        pg.evaluate("()=>window.__harness.hub.reset()")
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "calendar", "size": "l", "pinned": False}]})
        if doc is not None: pg.evaluate("d=>localStorage.setItem('%s', JSON.stringify(d))" % KEY, doc)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-calendar"][data-view]:not([data-view="loading"])'); pg.wait_for_timeout(300)
    def stored():
        pg.wait_for_timeout(700)
        return json.loads(pg.evaluate("localStorage.getItem('%s')" % KEY) or "null")
    def add(title, date=None, time=None, note=None):
        T("cal-add").click()
        T("cal-title-in").fill(title)
        if date: T("cal-date-in").fill(date)
        if time: T("cal-time-in").fill(time)
        if note: T("cal-note-in").fill(note)
        T("cal-save").click()

    def opening():
        fresh()
        expect(T("cal-title")).to_have_text("November 2026")
        assert pg.locator('[data-testid="cal-day"]').count() == 42
        expect(day("2026-11-23")).to_have_attribute("data-today", "true")
        expect(day("2026-11-23")).to_have_attribute("data-selected", "true")
        expect(pg.locator('.cal-wd').first).to_have_text("Mon")
        expect(T("cal-none")).to_be_visible()
    check("opens on the current month with today highlighted, Monday first, and an empty agenda", opening)

    def adding():
        fresh()
        add("Dentist", "2026-11-25", "09:30", "Bring forms")
        expect(day("2026-11-25")).to_have_attribute("data-busy", "true")
        expect(day("2026-11-25")).to_have_attribute("data-selected", "true")
        ev = T("cal-event"); expect(ev).to_have_count(1)
        expect(ev.locator(".ce-when")).to_have_text("09:30")
        expect(ev).to_contain_text("Dentist")
        d = stored(); assert d["events"][0]["title"] == "Dentist" and d["events"][0]["date"] == "2026-11-25", d
    check("adding an event shows a dot, selects its day, lists it and saves it", adding)

    def validation():
        fresh()
        T("cal-add").click(); T("cal-save").click()
        expect(T("cal-problem")).to_be_visible()
        assert (stored() or {}).get("events", []) == []
        T("cal-cancel").click()
        expect(T("cal-form")).to_have_count(0)
    check("an event without a title is refused with a visible message", validation)

    def editing():
        fresh()
        add("Call", "2026-11-23", "10:00")
        T("cal-edit").click(); T("cal-title-in").fill("Call Sam"); T("cal-time-in").fill("11:15"); T("cal-save").click()
        expect(T("cal-event").locator(".ce-when")).to_have_text("11:15")
        expect(T("cal-event")).to_contain_text("Call Sam")
        assert len(stored()["events"]) == 1
        T("cal-delete").click()
        expect(T("cal-event")).to_have_count(0)
        pg.wait_for_timeout(700)
        assert stored()["events"] == []
    check("editing and deleting an event work and persist", editing)

    def navigation():
        fresh()
        T("cal-next").click(); expect(T("cal-title")).to_have_text("December 2026")
        T("cal-prev").click(); T("cal-prev").click(); expect(T("cal-title")).to_have_text("October 2026")
        T("cal-today").click(); expect(T("cal-title")).to_have_text("November 2026")
        expect(day("2026-11-23")).to_have_attribute("data-selected", "true")
    check("month navigation and Today work", navigation)

    def weekstart():
        fresh()
        T("cal-weekstart").click()
        expect(pg.locator('.cal-wd').first).to_have_text("Sun")
        expect(pg.locator('[data-testid="cal-day"]').first).to_have_attribute("data-date", "2026-11-01")
        pg.wait_for_timeout(700)
        assert stored()["weekStart"] == "sun"
        pg.reload(); pg.wait_for_selector('[data-testid="widget-calendar"][data-view="month"]')
        expect(pg.locator('.cal-wd').first).to_have_text("Sun")
    check("week start can be switched to Sunday and is remembered", weekstart)

    def keyboard():
        fresh()
        day("2026-11-23").focus()
        pg.keyboard.press("ArrowRight"); pg.keyboard.press("ArrowDown")
        expect(day("2026-12-01")).to_have_attribute("data-selected", "true")
        expect(T("cal-title")).to_have_text("December 2026")
        pg.keyboard.press("PageUp")
        expect(T("cal-title")).to_have_text("November 2026")
    check("arrow keys move the selected day and PageUp/PageDown change month", keyboard)

    def week_list():
        fresh({"version": 1, "weekStart": "mon", "next": 4, "events": [
            {"id": "e1", "date": "2026-11-23", "time": "18:00", "title": "Tonight", "note": ""},
            {"id": "e2", "date": "2026-11-24", "time": None, "title": "Tomorrow thing", "note": ""},
            {"id": "e3", "date": "2026-12-20", "time": None, "title": "Far away", "note": ""}]})
        T("cal-mode-week").click()
        expect(T("cal-event")).to_have_count(2)
        expect(T("cal-event").first.locator(".ce-when")).to_contain_text("Today")
        expect(T("cal-event").nth(1).locator(".ce-when")).to_contain_text("Tomorrow")
    check("the next-7-days list shows only upcoming events with relative days", week_list)

    def midnight():
        ctx2 = b.new_context(viewport={"width": 1100, "height": 760}, timezone_id="Asia/Kolkata", locale="en-GB")
        ctx2.add_init_script(clock(2026, 11, 23, 18, 29, 55))  # 23:59:55 in Kolkata
        q = ctx2.new_page(); q.goto(BASE + "/hub.html"); q.wait_for_selector('[data-testid="hub"]')
        q.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": [{"kind": "calendar", "size": "l", "pinned": False}]})
        q.reload(); q.wait_for_selector('[data-testid="widget-calendar"][data-view="month"]')
        d = lambda x: q.locator(f'[data-testid="cal-day"][data-date="{x}"]')
        expect(d("2026-11-23")).to_have_attribute("data-today", "true")
        q.wait_for_timeout(6500)
        expect(d("2026-11-24")).to_have_attribute("data-today", "true")
        ctx2.close()
    check("'today' moves on at local midnight without reopening", midnight)

    def damaged():
        fresh({"version": 1, "weekStart": "zzz", "next": "x", "events": [{"id": "e1", "date": "2026-02-31", "title": "bad"}, {"id": "e2", "date": "2026-11-23", "time": "25:00", "title": "ok date"}, 7]})
        expect(T("cal-title")).to_have_text("November 2026")
        assert pg.locator('[data-testid="cal-day"]').count() == 42
    check("damaged stored data is cleaned up instead of breaking the card", damaged)

    def no_errors():
        assert not errors, errors[:3]
    check("no console errors", no_errors)
    b.close()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
