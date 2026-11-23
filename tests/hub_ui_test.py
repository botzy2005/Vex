"""Widgets Hub framework checks (run against `npm run dev`, test backend with probe widgets)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:500])

def L(*kinds, size="s", pinned=()):
    return {"version": 1, "widgets": [{"kind": k, "size": size, "pinned": k in pinned} for k in kinds]}

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 1100, "height": 760})
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" and "probe exploded" not in m.text and "widget failed" not in m.text and "The above error" not in m.text else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    H = lambda js: pg.evaluate("()=>window.__harness.hub." + js)
    cards = pg.locator('[data-testid="widget-card"]')

    def order(): return pg.locator('[data-testid="widget-card"]').evaluate_all("els=>els.map(e=>e.dataset.kind)")
    def fresh(layout=None):
        pg.evaluate("()=>window.__harness.hub.reset()")
        if layout is not None: pg.evaluate("l=>window.__harness.hub.setLayout(l)", layout)
        pg.reload(); pg.wait_for_selector('[data-testid="hub"]'); pg.wait_for_timeout(250)
    def customise(on=True):
        btn = pg.locator('[data-testid="hub-customise"]')
        if (btn.get_attribute("aria-pressed") == "true") != on: btn.click()
    def card(kind): return pg.locator(f'[data-testid="widget-card"][data-kind="{kind}"]')

    # ---------------------------------------------------------------- first run, lazy loading
    def first_run():
        fresh()
        assert order() == ["welcome"], order()
        expect(pg.locator('[data-testid="widget-welcome"]')).to_be_visible()
        expect(pg.locator('[data-testid="hub-count"]')).to_have_text("1")
    check("first run shows only the Getting started card", first_run)

    def lazy_loading():
        fresh(L("probe-1", "probe-2"))
        pg.wait_for_selector('[data-testid="probe"]')
        loads = pg.evaluate("()=>window.__probe.loads")
        assert sorted(loads) == ["probe-1", "probe-2"], loads
        pg.locator('[data-testid="hub-add"]').click()
        pg.locator('[data-testid="gallery-item"][data-kind="probe-3"] [data-testid="gallery-add"]').click()
        pg.wait_for_selector('[data-testid="probe"][data-kind="probe-3"]')
        loads = pg.evaluate("()=>window.__probe.loads")
        assert sorted(loads) == ["probe-1", "probe-2", "probe-3"], loads
        assert "probe-4" not in loads
    check("a widget's code loads only when its card first shows", lazy_loading)

    def lifecycle_mount_unmount():
        fresh(L("probe-1"))
        pg.wait_for_selector('[data-testid="probe"]')
        assert pg.evaluate("()=>window.__probe.mounts['probe-1']") == 1
        customise(); card("probe-1").locator('[data-testid="card-remove"]').click()
        pg.wait_for_timeout(100)
        assert pg.evaluate("()=>window.__probe.unmounts['probe-1']") == 1
        assert pg.evaluate("()=>window.__probe.mounts['probe-1']") == 1
    check("removing a card unmounts its widget exactly once", lifecycle_mount_unmount)

    def active_flag():
        fresh(L("probe-1"))
        pg.wait_for_selector('[data-testid="probe"]')
        expect(pg.locator('[data-testid="probe"]')).to_have_attribute("data-active", "true")
        pg.evaluate("()=>{Object.defineProperty(document,'visibilityState',{value:'hidden',configurable:true});document.dispatchEvent(new Event('visibilitychange'))}")
        expect(pg.locator('[data-testid="probe"]')).to_have_attribute("data-active", "false")
        pg.evaluate("()=>{Object.defineProperty(document,'visibilityState',{value:'visible',configurable:true});document.dispatchEvent(new Event('visibilitychange'))}")
        expect(pg.locator('[data-testid="probe"]')).to_have_attribute("data-active", "true")
    check("widgets are told when the Hub is hidden (so they can stop work)", active_flag)

    # ---------------------------------------------------------------- gallery, add, remove
    def gallery_lists_catalog():
        fresh()
        pg.locator('[data-testid="hub-add"]').click()
        items = pg.locator('[data-testid="gallery-item"]')
        assert items.count() == 14, items.count()
        welcome = pg.locator('[data-testid="gallery-item"][data-kind="welcome"]')
        expect(welcome).to_have_attribute("data-added", "true")
        expect(welcome.locator('[data-testid="gallery-add"]')).to_be_disabled()
        pg.locator('[data-testid="hub-gallery-close"]').click()
        expect(pg.locator('[data-testid="hub-gallery"]')).to_have_count(0)
    check("gallery lists the catalog and marks what is already added", gallery_lists_catalog)

    def add_and_persist():
        fresh()
        pg.locator('[data-testid="hub-add"]').click()
        pg.locator('[data-testid="gallery-item"][data-kind="probe-2"] [data-testid="gallery-add"]').click()
        expect(card("probe-2")).to_be_visible()
        assert order() == ["welcome", "probe-2"]
        expect(pg.locator('[data-testid="gallery-item"][data-kind="probe-2"] [data-testid="gallery-add"]')).to_be_disabled()
        saved = H("saves")[-1]
        assert [w["kind"] for w in saved["widgets"]] == ["welcome", "probe-2"], saved
        pg.reload(); pg.wait_for_selector('[data-testid="widget-card"]')
        assert order() == ["welcome", "probe-2"]
    check("adding a widget appends it, saves it and survives a reload", add_and_persist)

    def remove_and_empty():
        fresh(L("probe-1", "probe-2"))
        customise()
        card("probe-1").locator('[data-testid="card-remove"]').click()
        assert order() == ["probe-2"]
        expect(pg.locator('[data-testid="hub-notice"]')).to_contain_text("removed")
        pg.locator('[data-testid="hub-add"]').click()
        expect(pg.locator('[data-testid="gallery-item"][data-kind="probe-1"] [data-testid="gallery-add"]')).to_be_enabled()
        card("probe-2").locator('[data-testid="card-remove"]').click()
        expect(pg.locator('[data-testid="hub-empty"]')).to_be_visible()
        pg.reload(); pg.wait_for_selector('[data-testid="hub-empty"]')
        assert order() == []
        pg.locator('[data-testid="hub-empty-add"]').click()
        expect(pg.locator('[data-testid="hub-gallery"]')).to_be_visible()
    check("removing works, an empty dashboard stays empty after a reload, and offers Add", remove_and_empty)

    def remove_keeps_data():
        fresh(L("probe-1"))
        pg.locator('[data-testid="probe-input"]').fill("keep me")
        pg.wait_for_timeout(700)
        customise(); card("probe-1").locator('[data-testid="card-remove"]').click()
        pg.locator('[data-testid="hub-add"]').click()
        pg.locator('[data-testid="gallery-item"][data-kind="probe-1"] [data-testid="gallery-add"]').click()
        expect(pg.locator('[data-testid="probe-input"]')).to_have_value("keep me")
    check("removing a card does not delete the widget's data", remove_keeps_data)

    def reset_layout():
        fresh(L("probe-1", "probe-2", "welcome"))
        customise()
        pg.locator('[data-testid="hub-reset"]').click()
        assert order() == ["probe-1", "probe-2", "welcome"]  # asks first
        pg.locator('[data-testid="hub-reset-yes"]').click()
        assert order() == ["welcome"], order()
    check("Reset layout asks first, then restores the default", reset_layout)

    # ---------------------------------------------------------------- resize
    def resize():
        fresh(L("probe-1", "welcome"))
        customise()
        c = card("probe-1")
        w0 = c.bounding_box()["width"]
        c.locator('[data-testid="size-l"]').click()
        expect(c).to_have_attribute("data-size", "l")
        b1 = c.bounding_box()
        assert b1["width"] > w0 * 1.8, (w0, b1)
        assert b1["height"] > 300, b1
        assert c.locator('[data-testid="size-s"]').count() == 1
        assert card("welcome").locator('[data-testid="size-s"]').count() == 0, "welcome only offers its allowed sizes"
        assert H("saves")[-1]["widgets"][0]["size"] == "l"
    check("resizing uses only the sizes a widget allows and changes the card's cells", resize)

    # ---------------------------------------------------------------- pin
    def pin_limit():
        fresh(L("probe-1", "probe-2", "probe-3", "probe-4", "probe-5"))
        for k in ("probe-1", "probe-2", "probe-3", "probe-4"):
            card(k).locator('[data-testid="card-pin"]').click()
        for k in ("probe-1", "probe-2", "probe-3", "probe-4"):
            expect(card(k)).to_have_attribute("data-pinned", "true")
        card("probe-5").locator('[data-testid="card-pin"]').click()
        expect(card("probe-5")).to_have_attribute("data-pinned", "false")
        expect(pg.locator('[data-testid="hub-notice"]')).to_contain_text("up to 4")
        card("probe-1").locator('[data-testid="card-pin"]').click()
        card("probe-5").locator('[data-testid="card-pin"]').click()
        expect(card("probe-5")).to_have_attribute("data-pinned", "true")
        pinned = [w["kind"] for w in H("layout()")["widgets"] if w["pinned"]]
        assert sorted(pinned) == ["probe-2", "probe-3", "probe-4", "probe-5"], pinned
        assert order() == ["probe-1", "probe-2", "probe-3", "probe-4", "probe-5"], "pinning does not reorder"
    check("pinning is limited to four, unpinning frees a slot, order is unchanged", pin_limit)

    # ---------------------------------------------------------------- rearranging
    def reorder_buttons():
        fresh(L("probe-1", "probe-2", "probe-3"))
        customise()
        card("probe-1").locator('[data-testid="card-later"]').click()
        assert order() == ["probe-2", "probe-1", "probe-3"], order()
        card("probe-3").locator('[data-testid="card-earlier"]').click()
        assert order() == ["probe-2", "probe-3", "probe-1"], order()
        expect(card("probe-2").locator('[data-testid="card-earlier"]')).to_be_disabled()
        expect(card("probe-1").locator('[data-testid="card-later"]')).to_be_disabled()
        pg.reload(); pg.wait_for_selector('[data-testid="widget-card"]')
        assert order() == ["probe-2", "probe-3", "probe-1"], "saved order is restored"
    check("move earlier / later buttons reorder and persist", reorder_buttons)

    def reorder_keyboard():
        fresh(L("probe-1", "probe-2", "probe-3"))
        customise()
        card("probe-1").focus()
        pg.keyboard.press("Alt+ArrowRight"); pg.wait_for_timeout(150)
        assert order() == ["probe-2", "probe-1", "probe-3"], order()
        assert pg.evaluate("document.activeElement?.dataset.kind") == "probe-1", "focus follows the moved card"
        pg.keyboard.press("Alt+ArrowRight"); pg.wait_for_timeout(150)
        pg.keyboard.press("Alt+ArrowLeft"); pg.wait_for_timeout(150)
        assert order() == ["probe-2", "probe-1", "probe-3"], order()
        assert "moved to position" in pg.locator('[data-testid="hub-announce"]').inner_text()
        card("probe-2").focus(); pg.keyboard.press("Alt+ArrowLeft"); pg.wait_for_timeout(100)
        assert order()[0] == "probe-2", "cannot move past the start"
    check("Alt+arrow keys reorder the focused card, focus stays on it, and it is announced", reorder_keyboard)

    def reorder_drag():
        fresh(L("probe-1", "probe-2", "probe-3", "probe-4"))
        customise()
        g = card("probe-1").locator('[data-testid="card-grip"]').bounding_box()
        t = card("probe-4").bounding_box()
        pg.mouse.move(g["x"] + g["width"] / 2, g["y"] + g["height"] / 2)
        pg.mouse.down()
        pg.mouse.move(t["x"] + t["width"] / 2, t["y"] + t["height"] / 2, steps=8)
        expect(pg.locator('[data-testid="drag-label"]')).to_be_visible()
        expect(card("probe-1")).to_have_attribute("data-dragging", "true")
        pg.wait_for_timeout(150)
        assert order() == ["probe-2", "probe-3", "probe-4", "probe-1"], ("live preview", order())
        assert H("saves") == [], "nothing is saved until the drop"
        pg.mouse.up()
        expect(pg.locator('[data-testid="drag-label"]')).to_have_count(0)
        assert order() == ["probe-2", "probe-3", "probe-4", "probe-1"], order()
        assert [w["kind"] for w in H("layout()")["widgets"]] == ["probe-2", "probe-3", "probe-4", "probe-1"]
        pg.reload(); pg.wait_for_selector('[data-testid="widget-card"]')
        assert order() == ["probe-2", "probe-3", "probe-4", "probe-1"]
    check("dragging a card by its handle reorders it live, saves on drop, and persists", reorder_drag)

    def drag_escape_cancels():
        fresh(L("probe-1", "probe-2", "probe-3"))
        customise()
        g = card("probe-1").locator('[data-testid="card-grip"]').bounding_box()
        t = card("probe-3").bounding_box()
        pg.mouse.move(g["x"] + 5, g["y"] + 5); pg.mouse.down()
        pg.mouse.move(t["x"] + 40, t["y"] + 40, steps=6)
        pg.keyboard.press("Escape"); pg.mouse.up(); pg.wait_for_timeout(100)
        assert order() == ["probe-1", "probe-2", "probe-3"], order()
        assert H("saves") == []
    check("Escape cancels a drag", drag_escape_cancels)

    def no_grip_outside_customise():
        fresh(L("probe-1"))
        assert pg.locator('[data-testid="card-grip"]').count() == 0
        assert pg.locator('[data-testid="card-remove"]').count() == 0
        customise()
        assert pg.locator('[data-testid="card-grip"]').count() == 1
        pg.keyboard.press("Escape")
        expect(pg.locator('[data-testid="hub-customise"]')).to_have_attribute("aria-pressed", "false")
    check("cards are calm outside Customise (no handles or remove) and Escape leaves it", no_grip_outside_customise)

    # ---------------------------------------------------------------- data
    def data_persists():
        fresh(L("probe-1"))
        pg.locator('[data-testid="probe-input"]').fill("hello hub")
        pg.wait_for_timeout(800)
        assert H("data('probe-1')") == {"text": "hello hub"}
        pg.reload(); pg.wait_for_selector('[data-testid="probe-input"]'); pg.wait_for_timeout(200)
        expect(pg.locator('[data-testid="probe-input"]')).to_have_value("hello hub")
    check("a widget's data is saved after a pause and restored on reload", data_persists)

    def data_flushes_on_unmount():
        fresh(L("probe-1"))
        pg.locator('[data-testid="probe-input"]').fill("quick")
        customise(); card("probe-1").locator('[data-testid="card-remove"]').click()  # well inside the 400 ms debounce
        pg.wait_for_timeout(100)
        assert H("data('probe-1')") == {"text": "quick"}, H("data('probe-1')")
    check("unsaved widget data is saved when the widget goes away", data_flushes_on_unmount)

    def data_rapid_typing_batches():
        fresh(L("probe-1"))
        i = pg.locator('[data-testid="probe-input"]')
        i.click(); pg.keyboard.type("abcdefghij", delay=15)
        pg.wait_for_timeout(900)
        writes = H("dataWrites")
        assert 1 <= len(writes) <= 2, len(writes)
        assert writes[-1]["value"] == {"text": "abcdefghij"}
    check("rapid typing is saved as one write, not one per keystroke", data_rapid_typing_batches)

    # ---------------------------------------------------------------- failure isolation
    def crash_isolated():
        fresh(L("welcome", "probe-1", "probe-crash"))
        pg.evaluate("()=>{window.__hubCrash=true}")
        pg.reload(); pg.wait_for_selector('[data-testid="widget-error"]')
        expect(pg.locator('[data-testid="widget-error"]')).to_have_count(1)
        expect(pg.locator('[data-testid="widget-welcome"]')).to_be_visible()
        expect(pg.locator('[data-testid="probe"][data-kind="probe-1"]')).to_be_visible()
        expect(pg.locator('[data-testid="widget-error"]')).to_contain_text("probe exploded")
        pg.locator('[data-testid="probe-input"]').fill("still typing")
        pg.wait_for_timeout(600)
        assert H("data('probe-1')") == {"text": "still typing"}
    check("a widget that throws shows an error on its own card; the others keep working", crash_isolated)

    def crash_reload_and_remove():
        fresh(L("welcome", "probe-crash"))
        pg.evaluate("()=>{window.__hubCrash=true}")
        pg.reload(); pg.wait_for_selector('[data-testid="widget-error"]')
        pg.evaluate("()=>{window.__hubCrash=false}")
        pg.locator('[data-testid="widget-error"] >> text=Reload').click()
        pg.wait_for_selector('[data-testid="probe"][data-kind="probe-crash"]')
        pg.evaluate("()=>{window.__hubCrash=true}")
        pg.reload(); pg.wait_for_selector('[data-testid="widget-error"]')
        pg.locator('[data-testid="widget-error"] >> text=Remove').click()
        assert order() == ["welcome"], order()
    check("Reload re-runs a failed widget; Remove drops it from the error card", crash_reload_and_remove)

    def save_failure_shown():
        fresh(L("probe-1"))
        H("failNextSave('disk full')")
        customise(); card("probe-1").locator('[data-testid="card-remove"]').click()
        expect(pg.locator('[data-testid="hub-notice"]')).to_contain_text("disk full")
        expect(pg.locator('[data-testid="hub-notice"]')).to_have_attribute("data-error", "true")
    check("a failed save is reported, not swallowed", save_failure_shown)

    def data_failure_does_not_crash():
        fresh(L("probe-1"))
        H("failNextData('no space')")
        pg.locator('[data-testid="probe-input"]').fill("x")
        pg.wait_for_timeout(700)
        expect(pg.locator('[data-testid="probe-input"]')).to_have_value("x")
        expect(pg.locator('[data-testid="widget-error"]')).to_have_count(0)
    check("a failed data save leaves the widget usable", data_failure_does_not_crash)

    # ---------------------------------------------------------------- multi-window and layout
    def other_window_changes():
        fresh(L("probe-1"))
        other = ctx.new_page(); other.goto(BASE + "/hub.html"); other.wait_for_selector('[data-testid="hub"]')
        other.evaluate("l=>window.__harness.hub.setLayout(l)", L("probe-1", "probe-2", pinned=("probe-2",)))
        pg.wait_for_timeout(300)
        assert order() == ["probe-1", "probe-2"], order()
        expect(card("probe-2")).to_have_attribute("data-pinned", "true")
        other.close()
    check("a layout change made elsewhere (island, another window) appears here", other_window_changes)

    def narrow_window_two_columns():
        fresh(L("probe-1", "probe-2", "probe-3"))
        cols = lambda: len(pg.evaluate("getComputedStyle(document.querySelector('[data-testid=hub-grid]')).gridTemplateColumns").split(" "))
        assert cols() == 4, cols()
        pg.set_viewport_size({"width": 700, "height": 760}); pg.wait_for_timeout(150)
        assert cols() == 2, cols()
        for c in cards.all():
            bb = c.bounding_box(); assert bb["x"] >= 0 and bb["x"] + bb["width"] <= 700 + 1, bb
        pg.set_viewport_size({"width": 1100, "height": 760})
    check("the grid reflows to two columns in a narrow window with nothing off-screen", narrow_window_two_columns)

    # ---------------------------------------------------------------- the island side
    # same browser context as the Hub page, so storage and the broadcast channel are shared like the real windows share the native side
    ip = ctx.new_page(); ip.set_viewport_size({"width": 700, "height": 420})
    ierrors = []
    ip.on("pageerror", lambda e: ierrors.append(str(e)))
    ip.goto(BASE + "/index.html"); ip.wait_for_selector('[data-testid="island"]')
    IH = lambda js, *a: ip.evaluate("a=>window.__harness.hub." + js, *a) if a else ip.evaluate("()=>window.__harness.hub." + js)
    isl = ip.locator('[data-testid="island"]')

    def island_reset(layout=None):
        ip.evaluate("()=>window.__harness.hub.reset()")
        if layout is not None: ip.evaluate("l=>window.__harness.hub.setLayout(l)", layout)
        ip.reload(); ip.wait_for_selector('[data-testid="island"]')
        isl.hover(); ip.wait_for_timeout(900)
        ip.locator('[data-testid="tab-widgets"]').click(); ip.wait_for_timeout(500)

    def widgets_tab_empty():
        island_reset()
        expect(ip.locator('[data-testid="widgets-tab"]')).to_be_visible()
        expect(ip.locator('[data-testid="widgets-empty"]')).to_be_visible()
        assert ip.locator('[data-testid="island-timers"]').count() == 0
        bb = isl.bounding_box(); assert bb["height"] > 250, bb
    check("island: a Widgets tab with a clear empty state", widgets_tab_empty)

    def open_hub_button():
        island_reset()
        ip.locator('[data-testid="open-hub"]').click()
        assert IH("opens") == [None], IH("opens")
    check("island: Open Hub asks for the Hub window", open_hub_button)

    def pinned_tiles_and_open():
        island_reset(L("probe-1", "probe-2", "probe-3", pinned=("probe-2", "probe-3")))
        tiles = ip.locator('[data-testid="pinned-widget"]')
        assert tiles.count() == 2
        assert ip.locator('[data-testid="pinned-widget"]').evaluate_all("e=>e.map(x=>x.dataset.kind)") == ["probe-2", "probe-3"]
        tiles.nth(1).click()
        assert IH("opens") == ["probe-3"], IH("opens")
    check("island: pinned widgets are one click away and open the Hub on that widget", pinned_tiles_and_open)

    def hub_follows_island_click():
        # the real chain: island tile -> hub window scrolls to / flashes that widget
        fresh(L("probe-1", "probe-2", "probe-3", pinned=("probe-3",)))
        island_reset(L("probe-1", "probe-2", "probe-3", pinned=("probe-3",)))
        ip.locator('[data-testid="pinned-widget"]').click()
        expect(card("probe-3")).to_have_attribute("data-flash", "true", timeout=2000)
    check("an already-open Hub highlights the widget chosen on the island", hub_follows_island_click)

    def layout_live_on_island():
        island_reset(L("probe-1", "probe-2"))
        assert ip.locator('[data-testid="pinned-widget"]').count() == 0
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", L("probe-1", "probe-2", pinned=("probe-1",)))
        pg.reload(); pg.wait_for_selector('[data-testid="widget-card"]'); pg.wait_for_timeout(200)
        customise(False)
        card("probe-2").locator('[data-testid="card-pin"]').click()
        ip.wait_for_timeout(400)
        assert ip.locator('[data-testid="pinned-widget"]').count() == 2, ip.locator('[data-testid="pinned-widget"]').count()
    check("island: pinning in the Hub updates the island straight away", layout_live_on_island)

    def statuses_shown():
        island_reset(L("probe-1", "probe-2"))
        ip.evaluate("()=>window.__harness.hub.api.hubSetStatus('probe-1',{label:'Low priority',detail:'quiet',priority:0})")
        ip.evaluate("()=>window.__harness.hub.api.hubSetStatus('probe-2',{label:'  Urgent\\n thing ',detail:null,priority:3})")
        ip.wait_for_timeout(200)
        labels = ip.locator('[data-testid="widget-status"] .wt-status-label').all_inner_texts()
        assert labels == ["Urgent thing", "Low priority"], labels
        ip.locator('[data-testid="widget-status"]').first.click()
        assert IH("opens")[-1] == "probe-2"
        ip.evaluate("()=>window.__harness.hub.api.hubSetStatus('probe-2',null)")
        ip.wait_for_timeout(200)
        assert ip.locator('[data-testid="widget-status"]').count() == 1
        rejected = ip.evaluate("async()=>{try{await window.__harness.hub.api.hubSetStatus('nope',{label:'x',priority:0});return 'accepted'}catch(e){return String(e)}}")
        assert "does not exist" in rejected, rejected
    check("island: widget status lines show by priority, are cleaned, clear, and unknown widgets are refused", statuses_shown)

    def timers_on_island():
        island_reset()
        ip.evaluate("()=>window.__harness.hub.api.timerCreate('welcome','Tea',3000).then(id=>window.__harness.hub.api.timerStart(id))")
        ip.wait_for_timeout(400)
        row = ip.locator('[data-testid="island-timer"]')
        expect(row).to_have_count(1)
        txt = row.inner_text(); assert "Tea" in txt and ":0" in txt, txt
        t1 = ip.locator('[data-testid="island-timer"] .wt-timer-time').inner_text()
        ip.wait_for_timeout(1300)
        t2 = ip.locator('[data-testid="island-timer"] .wt-timer-time').inner_text()
        assert t1 != t2, (t1, t2)
        ip.wait_for_timeout(2200)
        expect(ip.locator('[data-testid="island-timer"]')).to_have_attribute("data-state", "finished")
        expect(ip.locator('[data-testid="island-timer"] .wt-timer-time')).to_have_text("Done")
    check("island: a running shared timer counts down and shows Done when it ends", timers_on_island)

    def timer_pause_and_idle_hidden():
        island_reset()
        ip.evaluate("()=>window.__harness.hub.api.timerCreate('welcome','Idle one',60000)")
        ip.wait_for_timeout(300)
        assert ip.locator('[data-testid="island-timer"]').count() == 0, "idle timers are not shown"
        ip.evaluate("async()=>{const a=window.__harness.hub.api;const l=await a.timersList();await a.timerStart(l[0].id);await a.timerPause(l[0].id)}")
        ip.wait_for_timeout(300)
        txt = ip.locator('[data-testid="island-timer"]').inner_text(); assert "Paused" in txt and "1:00" in txt or "0:59" in txt, txt
    check("island: idle timers stay hidden; a paused one shows its remaining time", timer_pause_and_idle_hidden)

    def no_tick_when_nothing_runs():
        island_reset()
        n = ip.evaluate("""async()=>{let c=0;const o=window.setInterval;window.setInterval=(...a)=>{c++;return o(...a)};await new Promise(r=>setTimeout(r,2500));window.setInterval=o;return c}""")
        assert n == 0, n
    check("island: no interval is started while no timer is running", no_tick_when_nothing_runs)

    def collapsing_island_unaffected():
        island_reset(L("probe-1", pinned=("probe-1",)))
        ip.mouse.move(5, 400); ip.wait_for_timeout(1500)
        assert isl.bounding_box()["height"] < 80, isl.bounding_box()
        isl.hover(); ip.wait_for_timeout(900)
        for t in ("music", "lyrics", "assistant", "notifications", "clipboard", "screenshot", "widgets"):
            expect(ip.locator(f'[data-testid="tab-{t}"]')).to_be_visible()
    check("island: collapse/expand and every existing tab still work with the new tab", collapsing_island_unaffected)

    check("island: no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(ierrors[:3])) if ierrors else None)

    pg.evaluate("()=>window.__harness.hub.reset()")
    if errors: print("ERRORS:", errors[:5])
    check("no uncaught page errors or console errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} hub checks passed")
sys.exit(0 if passed == len(results) else 1)
