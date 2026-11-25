"""VEX Connect window and settings checks (run against `npm run dev`, test backend)."""
import sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:500])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 520, "height": 720})
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.goto(BASE + "/connect.html"); pg.wait_for_selector('[data-testid="cx-root"]')
    T = lambda t: pg.locator(f'[data-testid="{t}"]')
    C = lambda js, *a: pg.evaluate("a=>window.__harness.connect." + js, *a) if a else pg.evaluate("()=>window.__harness.connect." + js)

    def fresh(enabled=True):
        pg.evaluate("()=>window.__harness.connect.reset()")
        pg.evaluate("e=>window.__harness.patchSettings ? 0 : 0", enabled)
        pg.evaluate("localStorage.clear()")
        pg.reload(); pg.wait_for_selector('[data-testid="cx-root"]')
        if enabled:
            T("cx-enabled").click(); T("cx-pair").wait_for()

    def pair_phone(name="OnePlus Nord"):
        T("cx-pair").click(); T("cx-qr").wait_for()
        C("phoneScans(a)", name) if False else pg.evaluate("n=>window.__harness.connect.phoneScans(n)", name)
        T("cx-sas").wait_for()

    def off_by_default():
        pg.evaluate("localStorage.clear()"); pg.reload(); pg.wait_for_selector('[data-testid="cx-root"]')
        expect(T("cx-off")).to_be_visible()
        assert T("cx-enabled").get_attribute("aria-checked") == "false"
        assert T("cx-pair").count() == 0
    check("VEX Connect is off by default and says nothing listens", off_by_default)

    def turn_on():
        T("cx-enabled").click(); expect(T("cx-pair")).to_be_visible()
        assert "Listening" in T("cx-listening").inner_text()
        expect(T("cx-empty")).to_be_visible()
    check("turning it on shows pairing and an empty device list", turn_on)

    def qr_flow():
        fresh()
        T("cx-pair").click()
        expect(T("cx-qr")).to_be_visible(); expect(T("qr-svg")).to_be_visible()
        assert "Expires in" in T("cx-countdown").inner_text()
        T("cx-codetext").evaluate("e=>e.closest('details').open=true")
        assert T("cx-codetext").input_value().startswith("vexlink:")
    check("a QR code, countdown and paste-code fallback are shown", qr_flow)

    def confirm_flow():
        fresh(); pair_phone()
        assert T("cx-sas").inner_text().strip() == "482 913"
        assert T("cx-qr").count() == 0, "the code is hidden once it was used"
        T("cx-yes").click()
        expect(T("cx-waiting")).to_be_visible()
        assert T("cx-devices").count() == 0, "not paired until the phone confirms too"
        pg.evaluate("()=>window.__harness.connect.phoneAnswers(true)")
        expect(T("cx-pairing")).to_have_attribute("data-state", "paired")
        T("cx-done").click()
        expect(T("cx-device-name")).to_have_text("OnePlus Nord")
    check("pairing completes only after both sides confirm", confirm_flow)

    def decline_pc():
        fresh(); pair_phone()
        T("cx-no").click()
        expect(T("cx-pairing")).to_have_attribute("data-state", "declined")
        assert pg.evaluate("()=>window.__harness.connect.devices.length") == 0
        T("cx-retry").click(); expect(T("cx-qr")).to_be_visible()
    check("declining on the PC pairs nothing and offers a new code", decline_pc)

    def decline_phone():
        fresh(); pair_phone(); T("cx-yes").click()
        pg.evaluate("()=>window.__harness.connect.phoneAnswers(false)")
        expect(T("cx-pairing")).to_have_attribute("data-state", "declined")
        assert pg.evaluate("()=>window.__harness.connect.devices.length") == 0
    check("declining on the phone pairs nothing", decline_phone)

    def expired():
        fresh(); T("cx-pair").click(); T("cx-qr").wait_for()
        pg.evaluate("()=>window.__harness.connect.expireCode()")
        expect(T("cx-pairing")).to_have_attribute("data-state", "expired")
        assert T("cx-qr").count() == 0
        expect(T("cx-pairing-message")).to_contain_text("expired")
    check("an expired code is replaced by a clear message", expired)

    def cancel():
        fresh(); T("cx-pair").click(); T("cx-qr").wait_for(); T("cx-cancel").click()
        expect(T("cx-pair")).to_be_visible()
    check("cancelling returns to the start", cancel)

    def finish_pair(name):
        pair_phone(name); T("cx-yes").click()
        pg.evaluate("()=>window.__harness.connect.phoneAnswers(true)"); T("cx-done").click()

    def list_actions():
        fresh(); finish_pair("Pixel"); finish_pair("Pixel")
        names = T("cx-device-name").all_inner_texts()
        assert names == ["Pixel", "Pixel (2)"], names
        dev = T("cx-device").first
        dev_id = dev.get_attribute("data-id")
        assert T("cx-disconnect").count() == 0
        pg.evaluate("id=>window.__harness.connect.setConnected(id,true)", dev_id)
        expect(dev.locator('[data-testid="cx-device-status"]')).to_have_text("Connected")
        dev.locator('[data-testid="cx-disconnect"]').click()
        expect(dev.locator('[data-testid="cx-device-status"]')).not_to_have_text("Connected")
        # rename
        dev.locator('[data-testid="cx-rename"]').click()
        inp = T("cx-rename-input"); inp.fill("My phone"); inp.press("Enter")
        expect(T("cx-device-name").first).to_have_text("My phone")
        # remove needs a second click
        dev.locator('[data-testid="cx-remove"]').click()
        assert T("cx-device").count() == 2
        dev.locator('[data-testid="cx-remove-no"]').click(); assert T("cx-device").count() == 2
        dev.locator('[data-testid="cx-remove"]').click(); dev.locator('[data-testid="cx-remove-yes"]').click()
        expect(T("cx-device")).to_have_count(1)
        assert "revoke:" + dev_id in pg.evaluate("()=>window.__harness.connect.calls")
    check("duplicate names, disconnect, rename and confirmed removal", list_actions)

    def errors_shown():
        fresh()
        pg.evaluate("()=>window.__harness.connect.failNext('Something went wrong.')")
        T("cx-pair").click()
        expect(T("cx-error")).to_have_text("Something went wrong.")
    check("a failed action shows its reason", errors_shown)

    def port_error():
        fresh(); pg.evaluate("()=>window.__harness.connect.setError('Could not listen on port 47821: in use.')")
        expect(T("cx-status-error")).to_contain_text("port 47821")
        assert T("cx-pair").is_disabled()
    check("a port that cannot be opened is explained and blocks pairing", port_error)

    def turn_off():
        fresh(); T("cx-enabled").click(); expect(T("cx-off")).to_be_visible()
    check("turning it off hides pairing again", turn_off)

    def settings_page():
        pg.goto(BASE + "/settings.html"); pg.wait_for_selector("nav, [role=tablist], button")
        pg.get_by_role("button", name="Phone").click()
        expect(T("cx-privacy")).to_be_visible()
        assert T("cx-set-enabled").get_attribute("aria-checked") == "false"
        T("cx-set-name").fill("Study PC"); T("cx-set-name").press("Enter")
        T("cx-set-enabled").click()
        pg.wait_for_timeout(200)
        assert T("cx-set-enabled").get_attribute("aria-checked") == "true"
        T("cx-set-open").click()
        assert "open" in pg.evaluate("()=>window.__harness.connect.calls")
    check("Settings has a Phone page with the toggle, name and an Open button", settings_page)

    check("no page errors", lambda: (_ for _ in ()).throw(AssertionError(errors)) if errors else None)
    b.close()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
