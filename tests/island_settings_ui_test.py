"""Settings > Island section checks (run against `npm run dev`, test backend)."""
import sys, json
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:500])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 900, "height": 700})
    errors = []
    isl = ctx.new_page(); isl.on("pageerror", lambda e: errors.append(str(e)))
    isl.goto(BASE + "/index.html"); isl.wait_for_selector('[data-testid="island"]')
    isl.evaluate("()=>localStorage.removeItem('islet-test-settings')"); isl.reload(); isl.wait_for_timeout(300)
    s = ctx.new_page(); s.on("pageerror", lambda e: errors.append(str(e)))
    s.goto(BASE + "/settings.html"); s.wait_for_selector('[data-testid="page-island"]')
    s.locator('[data-testid="page-island"]').click(); s.wait_for_timeout(300)
    T = lambda t: s.locator(f'[data-testid="{t}"]')
    DEF = {"volumeButton": True, "volumeStep": 5, "visibility": "always", "autoHideDelayMs": 800, "offsetX": 0}
    def saved():
        raw = s.evaluate("localStorage.getItem('islet-test-settings')")
        return {**DEF, **(json.loads(raw).get("island", {}) if raw else {})}

    def groups():
        order = s.evaluate("()=>[...document.querySelectorAll('[data-testid^=group-]')].map(e=>e.dataset.testid)")
        assert order == ["group-position", "group-visibility", "group-behaviour", "group-widgets", "group-volume"], order
        # each control sits under its own group heading
        pos = s.evaluate("""()=>{const y=id=>document.querySelector('[data-testid="'+id+'"]').getBoundingClientRect().top;
          return {pos:y('group-position'),mon:y('monitor'),off:y('offset'),vis:y('group-visibility'),visb:y('visibility'),
                  beh:y('group-behaviour'),hov:y('hover'),vol:y('group-volume'),vb:y('volume-button'),vs:y('volume-step')}}""")
        assert pos["pos"] < pos["mon"] < pos["off"] < pos["vis"] < pos["visb"] < pos["beh"] < pos["hov"] < pos["vol"] < pos["vb"] < pos["vs"], pos
    check("the Island page is grouped: Position, Visibility, Behaviour, Widgets, Volume", groups)

    def volume_controls():
        assert saved()["volumeButton"] is True and saved()["volumeStep"] == 5
        s.evaluate("()=>{const e=document.querySelector('[data-testid=\"volume-step\"]');const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;set.call(e,'12');e.dispatchEvent(new Event('input',{bubbles:true}))}")
        s.wait_for_timeout(300); assert saved()["volumeStep"] == 12, saved()
        T("volume-button").click(); s.wait_for_timeout(300)
        assert saved()["volumeButton"] is False
        assert T("volume-step").is_disabled()
        T("volume-button").click(); s.wait_for_timeout(300)
        assert saved()["volumeButton"] is True and not T("volume-step").is_disabled()
    check("volume button toggle and step persist; the step is disabled while the button is off", volume_controls)

    def button_reaches_island():
        isl.locator('[data-testid="island"]').hover(); isl.wait_for_timeout(900)
        expect(isl.locator('[data-testid="btn-volume"]')).to_be_visible()
        T("volume-button").click(); s.wait_for_timeout(500)
        assert isl.locator('[data-testid="btn-volume"]').count() == 0
        T("volume-button").click(); s.wait_for_timeout(500)
        expect(isl.locator('[data-testid="btn-volume"]')).to_be_visible()
    check("switching the volume button off and on in Settings updates the island", button_reaches_island)

    def step_reaches_island():
        isl.locator('[data-testid="btn-volume"]').click(); isl.wait_for_timeout(300)
        before = int(isl.locator('[data-testid="volume-slider"]').input_value())
        isl.locator('[data-testid="volume-up"]').click(); isl.wait_for_timeout(300)
        after = int(isl.locator('[data-testid="volume-slider"]').input_value())
        assert after - before == 12, (before, after)
    check("the configured step is what plus does on the island", step_reaches_island)

    def visibility_group():
        assert T("autohide-delay").count() == 0
        T("visibility-auto").click(); s.wait_for_timeout(300)
        assert T("autohide-delay").count() == 1 and saved()["visibility"] == "auto"
        T("visibility-always").click(); s.wait_for_timeout(300)
    check("the hide delay appears only in Auto-hide", visibility_group)

    def position_group():
        T("offset").evaluate("e=>{const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;set.call(e,'-150');e.dispatchEvent(new Event('input',{bubbles:true}))}")
        s.wait_for_timeout(300); assert saved()["offsetX"] == -150
        assert not T("reset-position").is_disabled()
        T("reset-position").click(); s.wait_for_timeout(300); assert saved()["offsetX"] == 0
    check("position slider and Reset position still work", position_group)

    def reset_section():
        T("visibility-auto").click(); T("volume-button").click(); s.wait_for_timeout(200)
        T("reset-island").click(); s.wait_for_timeout(400)
        v = saved()
        assert v["visibility"] == "always" and v["volumeButton"] is True and v["volumeStep"] == 5 and v["autoHideDelayMs"] == 800, v
    check("Reset section restores position, visibility and volume defaults", reset_section)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} island settings checks passed")
sys.exit(0 if passed == len(results) else 1)
