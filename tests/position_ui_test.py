"""Island positioning checks (run against `npm run dev`, test backend)."""
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
    grip = pg.locator('[data-testid="drag-grip"]')
    H = lambda js: pg.evaluate("()=>window.__harness." + js)

    def fresh(screen=1920, expand=True):
        pg.goto(BASE + "/index.html"); pg.wait_for_selector('[data-testid="island"]')
        pg.evaluate("()=>localStorage.removeItem('islet-test-settings')")
        pg.reload(); pg.wait_for_selector('[data-testid="island"]'); pg.wait_for_timeout(300)
        pg.evaluate("s=>window.__harness.setScreenWidth(s)", screen)
        if expand:
            isl.hover(); pg.wait_for_timeout(900)
            expect(isl).to_have_attribute("data-expanded", "true")

    def calls(): return H("offsetCalls")
    def moves(): return [c for c in calls() if not c["commit"]]
    def commits(): return [c for c in calls() if c["commit"]]
    def center(loc):
        bb = loc.bounding_box(); return bb["x"] + bb["width"] / 2, bb["y"] + bb["height"] / 2
    def drag_by(start, dx, steps=10, alt=False, release=True):
        if alt: pg.keyboard.down("Alt")
        pg.mouse.move(*start); pg.mouse.down()
        pg.mouse.move(start[0] + dx, start[1], steps=steps)
        if release: pg.mouse.up()
        if alt: pg.keyboard.up("Alt")
        pg.wait_for_timeout(150)

    # ---------------------------------------------------------------- the grip
    def grip_hidden_until_hover():
        fresh(expand=False)
        pg.mouse.move(5, 400); pg.wait_for_timeout(700)
        assert float(grip.evaluate("e=>getComputedStyle(e).opacity")) == 0
        isl.hover(); pg.wait_for_timeout(500)
        assert float(grip.evaluate("e=>getComputedStyle(e).opacity")) > 0.5
    check("the grip is invisible until the island is hovered", grip_hidden_until_hover)

    def grip_drag_moves_and_saves():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 120)
        assert len(commits()) == 1, calls()
        assert commits()[0]["applied"] == 120, commits()
        assert len(moves()) >= 1
        assert moves()[-1]["applied"] <= 120 and moves()[-1]["applied"] > 0
        assert H("offsetCalls").index(commits()[0]) == len(calls()) - 1, "the save is the last call"
    check("dragging the grip moves the island and saves the position once, on release", grip_drag_moves_and_saves)

    def position_persists_and_next_drag_starts_there():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 90)
        pg.reload(); pg.wait_for_selector('[data-testid="island"]')
        pg.evaluate("()=>{window.__harness.offsetCalls.length=0}")
        isl.hover(); pg.wait_for_timeout(900)
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 40)
        assert commits()[-1]["applied"] == 130, commits()
    check("the saved position survives a reload and the next drag continues from it", position_persists_and_next_drag_starts_there)

    def left_and_right_and_between():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), -250); a = commits()[-1]["applied"]
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 400); c = commits()[-1]["applied"]
        assert a == -250 and c == 150, (a, c)
    check("it can be left of centre, right of centre and anywhere between", left_and_right_and_between)

    # ---------------------------------------------------------------- the Alt gesture
    def alt_drag_on_surface():
        fresh()
        x, y = center(isl)
        drag_by((x + 90, y - 20), -70, alt=True)
        assert commits() and commits()[-1]["applied"] == -70, calls()
    check("Alt+drag on the island surface repositions it", alt_drag_on_surface)

    def plain_drag_does_nothing():
        fresh()
        x, y = center(isl)
        drag_by((x + 90, y - 20), -70, alt=False)
        assert calls() == [], calls()
    check("a plain drag (no Alt) on the island does nothing", plain_drag_does_nothing)

    def alt_click_is_still_a_click():
        fresh()
        pg.keyboard.down("Alt"); pg.locator('[data-testid="tab-clipboard"]').click(); pg.keyboard.up("Alt")
        pg.wait_for_timeout(300)
        expect(pg.locator('[data-testid="tab-clipboard"]')).to_have_attribute("aria-selected", "true")
        assert calls() == [], "no movement means no drag"
    check("an Alt+click without moving still presses the button and moves nothing", alt_click_is_still_a_click)

    def drag_does_not_press_buttons():
        fresh()
        t = center(pg.locator('[data-testid="tab-screenshot"]'))
        drag_by(t, -90, alt=True)
        expect(pg.locator('[data-testid="tab-screenshot"]')).to_have_attribute("aria-selected", "false")
        assert commits(), "it did drag"
        # and the next ordinary click works again
        pg.locator('[data-testid="tab-screenshot"]').click(); pg.wait_for_timeout(300)
        expect(pg.locator('[data-testid="tab-screenshot"]')).to_have_attribute("aria-selected", "true")
    check("ending an Alt+drag on a button does not press it, and the next click works", drag_does_not_press_buttons)

    def not_inside_text_field():
        fresh()
        pg.locator('[data-testid="tab-assistant"]').click(); pg.wait_for_timeout(500)
        inp = pg.locator('textarea, input[type=text], input:not([type])').first
        c = center(inp)
        inp.fill("hello world")
        drag_by(c, 60, alt=True)
        assert calls() == [], calls()
        expect(inp).to_have_value("hello world")
    check("Alt+drag does not start inside a text field", not_inside_text_field)

    def slider_inside_island_unaffected():
        fresh()
        # an ordinary press-drag with no Alt on any control must not move the island
        for tid in ("btn-pin", "tab-music"):
            c = center(pg.locator(f'[data-testid="{tid}"]'))
            drag_by(c, 30)
        assert calls() == [], calls()
    check("ordinary presses on island controls never move the island", slider_inside_island_unaffected)

    def tiny_move_is_not_a_drag():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 2, steps=2)
        assert calls() == [], calls()
    check("a press that moves only a pixel or two is not a drag", tiny_move_is_not_a_drag)

    # ---------------------------------------------------------------- limits
    def clamped_to_the_screen():
        fresh(screen=800)
        grip.hover(); pg.wait_for_timeout(300)
        w = H("windowSizes")[-1][0]
        limit = (800 - w) // 2
        drag_by(center(grip), 600, steps=12)
        assert commits()[-1]["applied"] == limit, (commits(), limit, w)
        assert all(abs(c["applied"]) <= limit for c in calls())
        pg.mouse.move(350, 40); isl.hover(); pg.wait_for_timeout(900); grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), -1500, steps=12)
        assert commits()[-1]["applied"] == -limit, commits()
    check("the island cannot be dragged past either edge of the screen", clamped_to_the_screen)

    def escape_cancels():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 80)
        before = commits()[-1]["applied"]
        pg.evaluate("()=>{window.__harness.offsetCalls.length=0}")
        grip.hover(); pg.wait_for_timeout(300)
        c = center(grip); pg.mouse.move(*c); pg.mouse.down(); pg.mouse.move(c[0] + 150, c[1], steps=8)
        pg.keyboard.press("Escape"); pg.mouse.up(); pg.wait_for_timeout(200)
        assert commits()[-1]["applied"] == before, (commits(), before)
        assert len(commits()) == 1
    check("Escape during a drag puts the island back where it was", escape_cancels)

    def stays_expanded_while_dragging():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        c = center(grip); pg.mouse.move(*c); pg.mouse.down(); pg.mouse.move(c[0] - 280, c[1], steps=10)
        pg.wait_for_timeout(900)
        expect(isl).to_have_attribute("data-expanded", "true")
        expect(isl).to_have_attribute("data-dragging", "true")
        pg.mouse.up(); pg.wait_for_timeout(200)
        expect(isl).to_have_attribute("data-dragging", "false")
    check("the island stays open and shows a grabbing state for the whole drag", stays_expanded_while_dragging)

    def moves_are_coalesced():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        c = center(grip); pg.mouse.move(*c); pg.mouse.down()
        for i in range(1, 81): pg.mouse.move(c[0] + i * 2, c[1])
        pg.mouse.up(); pg.wait_for_timeout(300)
        assert 1 <= len(moves()) <= 80, len(moves())
        assert commits()[-1]["applied"] == 160, commits()
    check("rapid pointer movement is batched and the final position is exact", moves_are_coalesced)

    # ---------------------------------------------------------------- not in the way
    def grip_overlaps_no_control():
        fresh()
        for tab in ("music", "lyrics", "assistant", "notifications", "clipboard", "screenshot", "widgets"):
            pg.locator(f'[data-testid="tab-{tab}"]').click(); pg.wait_for_timeout(450)
            isl.hover(); pg.wait_for_timeout(150)
            hit = pg.evaluate("""()=>{
              const g=document.querySelector('[data-testid=drag-grip]').getBoundingClientRect();
              const bad=[];
              document.querySelectorAll('.island button, .island input, .island textarea, .island select, .island [role=slider], .island a, .island [data-clickable]').forEach(e=>{
                if(e.dataset.testid==='drag-grip') return;
                const r=e.getBoundingClientRect(); if(r.width===0||r.height===0) return;
                const cs=getComputedStyle(e); if(cs.visibility==='hidden'||cs.display==='none') return;
                if(r.left<g.right&&r.right>g.left&&r.top<g.bottom&&r.bottom>g.top) bad.push(e.dataset.testid||e.className||e.tagName);
              });
              return bad;}""")
            assert hit == [], (tab, hit)
    check("the grip never overlaps a button, field or slider on any tab", grip_overlaps_no_control)

    def collapsed_clicks_still_work():
        fresh(expand=False)
        pg.mouse.move(5, 400); pg.wait_for_timeout(600)
        isl.click(); pg.wait_for_timeout(600)
        expect(isl).to_have_attribute("data-expanded", "true")
    check("clicking the collapsed island still expands it", collapsed_clicks_still_work)

    # ---------------------------------------------------------------- settings
    def settings_reset():
        s = ctx.new_page(); s.goto(BASE + "/settings.html"); s.wait_for_selector('[data-testid="page-island"]')
        s.locator('[data-testid="page-island"]').click()
        s.evaluate("()=>window.__harness.patchSettings({island:{offsetX:-180}})"); s.wait_for_timeout(300)
        expect(s.locator('[data-testid="offset"]').locator("xpath=..").locator(".slider-val")).to_have_text("180px left")
        s.locator('[data-testid="reset-position"]').click(); s.wait_for_timeout(300)
        expect(s.locator('[data-testid="offset"]').locator("xpath=..").locator(".slider-val")).to_have_text("Centre")
        expect(s.locator('[data-testid="reset-position"]')).to_be_disabled()
        s.close()
    check("Settings > Island shows the position and Reset position puts it back in the centre", settings_reset)

    def reset_reaches_island():
        fresh()
        grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 100)
        s = ctx.new_page(); s.goto(BASE + "/settings.html"); s.wait_for_selector('[data-testid="page-island"]')
        s.locator('[data-testid="page-island"]').click()
        s.locator('[data-testid="reset-position"]').click(); s.wait_for_timeout(300); s.close()
        pg.reload(); pg.wait_for_selector('[data-testid="island"]')
        pg.evaluate("()=>{window.__harness.offsetCalls.length=0}")
        isl.hover(); pg.wait_for_timeout(900); grip.hover(); pg.wait_for_timeout(300)
        drag_by(center(grip), 30)
        assert commits()[-1]["applied"] == 30, commits()
    check("after Reset position the next drag starts from the centre", reset_reaches_island)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} position checks passed")
sys.exit(0 if passed == len(results) else 1)
