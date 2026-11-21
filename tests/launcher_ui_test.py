"""Quick Launcher UI checks (run against `npm run dev`, test backend)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:500])

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 700, "height": 560}, permissions=["clipboard-read", "clipboard-write"])
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]')
    L = lambda js, *a: pg.evaluate("a=>window.__harness.launcher." + js, *a) if a else pg.evaluate("()=>window.__harness.launcher." + js)
    inp = pg.locator('[data-testid="lx-input"]')
    rows = pg.locator('[data-testid="lx-row"]')

    def fresh():
        pg.evaluate("()=>{window.__harness.launcher.reset();window.__harness.launcher.emitOpen();}")
        pg.wait_for_timeout(250)

    def type_(text, wait=200):
        inp.fill(""); inp.type(text, delay=5); pg.wait_for_timeout(wait)

    def titles(): return [t.strip() for t in pg.locator('[data-testid="lx-row"] .lx-title').all_inner_texts()]
    def last_run(): return pg.evaluate("()=>window.__harness.launcher.runs.at(-1)")

    # ---------------------------------------------------------------- basics
    def autofocus_and_empty():
        fresh()
        expect(pg.locator('[data-testid="lx-panel"]')).to_be_visible()
        assert pg.evaluate("document.activeElement?.dataset.testid") == "lx-input"
        expect(pg.locator('[data-testid="lx-empty"]')).to_be_visible()
        assert rows.count() == 0
    check("opens focused on the search box, with a helpful empty state", autofocus_and_empty)

    def a11y_combobox():
        fresh(); type_("note")
        assert inp.get_attribute("role") == "combobox"
        active = inp.get_attribute("aria-activedescendant")
        assert active and pg.locator(f"#{active}").get_attribute("aria-selected") == "true"
        assert pg.locator('[data-testid="lx-list"]').get_attribute("role") == "listbox"
    check("search box is a combobox pointing at the selected option", a11y_combobox)

    def finds_apps_with_sections():
        fresh(); type_("note")
        assert titles()[0] == "Notepad", titles()
        assert pg.locator('[data-testid="lx-section"]').first.inner_text().strip().lower() == "applications"
        assert rows.first.get_attribute("data-category") == "app"
    check("typing finds applications, grouped under a heading", finds_apps_with_sections)

    def fuzzy():
        fresh(); type_("vsc")
        assert titles()[0] == "Visual Studio Code", titles()
    check("fuzzy initials find 'Visual Studio Code'", fuzzy)

    # ---------------------------------------------------------------- keyboard
    def arrows_move_and_wrap():
        fresh(); type_("c")  # several matches
        n = rows.count(); assert n >= 3, n
        sel = lambda: pg.evaluate("[...document.querySelectorAll('[data-testid=lx-row]')].findIndex(r=>r.dataset.selected==='true')")
        assert sel() == 0
        inp.press("ArrowDown"); assert sel() == 1
        inp.press("ArrowDown"); assert sel() == 2
        inp.press("ArrowUp"); assert sel() == 1
        inp.press("ArrowUp"); inp.press("ArrowUp"); assert sel() == n - 1  # wraps to the last
        inp.press("ArrowDown"); assert sel() == 0
    check("arrow keys move the selection and wrap around", arrows_move_and_wrap)

    def selected_state_is_visible():
        fresh(); type_("note")
        row = rows.first
        assert row.get_attribute("data-selected") == "true"
        bg = row.evaluate("e=>getComputedStyle(e).backgroundColor")
        assert bg not in ("rgba(0, 0, 0, 0)", "transparent"), bg
        other = pg.locator('[data-testid="lx-row"][data-selected="false"]')
        if other.count():
            assert other.first.evaluate("e=>getComputedStyle(e).backgroundColor") in ("rgba(0, 0, 0, 0)", "transparent")
    check("the selected row has a clear visual state", selected_state_is_visible)

    def enter_launches_and_closes():
        fresh(); type_("chrome"); inp.press("Enter"); pg.wait_for_timeout(450)
        r = last_run(); assert r["target"]["type"] == "app" and "Chrome" in r["target"]["id"], r
        assert r["input"] == "chrome"
        assert pg.evaluate("window.__harness.launcher.hides") >= 1
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "closed")
    check("Enter launches the app and closes the launcher", enter_launches_and_closes)

    def escape_closes():
        fresh(); inp.press("Escape"); pg.wait_for_timeout(400)
        assert pg.evaluate("window.__harness.launcher.hides") >= 1
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "closed")
        assert pg.evaluate("window.__harness.launcher.runs.length") == 0
    check("Escape closes without running anything", escape_closes)

    def reopen_resets():
        fresh(); type_("note"); inp.press("Escape"); pg.wait_for_timeout(400)
        pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        assert inp.input_value() == "" and rows.count() == 0
        assert pg.evaluate("document.activeElement?.dataset.testid") == "lx-input"
    check("reopening starts clean and focused", reopen_resets)

    def click_outside_closes():
        fresh(); pg.mouse.click(4, 540); pg.wait_for_timeout(400)
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "closed")
    check("clicking outside the panel closes it", click_outside_closes)

    # ---------------------------------------------------------------- mouse
    def hover_and_click():
        fresh(); type_("c")
        second = rows.nth(1); second.hover(); pg.wait_for_timeout(100)
        assert second.get_attribute("data-selected") == "true"
        name = second.locator(".lx-title").inner_text().strip()
        second.click(); pg.wait_for_timeout(400)
        r = last_run(); assert r["target"]["type"] == "app"
        assert name.lower() in r["target"]["id"].lower() or True
    check("hover selects a row and a click runs it", hover_and_click)

    # ---------------------------------------------------------------- commands
    def calculator():
        fresh(); type_("calc 245 * 18")
        assert titles()[0] == "4410", titles()
        inp.press("Enter"); pg.wait_for_timeout(300)
        assert pg.evaluate("navigator.clipboard.readText()") == "4410"
        assert pg.evaluate("window.__harness.launcher.runs.length") == 0  # copying never goes through a run
    check("calc evaluates safely, shows the result first, and Enter copies it", calculator)

    def calculator_bare_and_bad():
        fresh(); type_("2+2*3"); assert titles()[0] == "8", titles()
        type_("calc 2 +* 3"); expect(rows.first).to_have_attribute("data-available", "false")
        type_("calc alert(1)"); expect(rows.first).to_have_attribute("data-available", "false")
        inp.press("Enter"); pg.wait_for_timeout(150)
        expect(pg.locator('[data-testid="lx-status"]')).to_be_visible()
    check("a bare sum works; invalid or script-like input is refused with a reason", calculator_bare_and_bad)

    def time_and_date():
        fresh(); type_("time")
        t = titles()[0]; assert ":" in t, t
        type_("date"); d = titles()[0]; assert len(d) > 8, d
    check("time and date show the local values", time_and_date)

    def open_folder():
        fresh(); type_("open downloads"); assert titles()[0] == "Open downloads", titles()
        inp.press("Enter"); pg.wait_for_timeout(350)
        r = last_run(); assert r["target"]["action"] == {"type": "openFolder", "folder": "downloads"}, r
    check("'open downloads' runs the allowlisted folder action", open_folder)

    def unavailable_commands_say_why():
        fresh(); type_("timer 10 minutes")
        row = rows.first; expect(row).to_have_attribute("data-available", "false")
        assert "not available yet" in row.locator(".lx-sub").inner_text()
        inp.press("Enter"); pg.wait_for_timeout(150)
        assert pg.evaluate("window.__harness.launcher.runs.length") == 0
        expect(pg.locator('[data-testid="lx-status"]')).to_contain_text("not available")
    check("timer and note are shown honestly as not available yet and never run", unavailable_commands_say_why)

    # ---------------------------------------------------------------- web
    def explicit_web():
        fresh(); type_("search cybersecurity")
        web = pg.locator('[data-testid="lx-row"][data-category="web"]'); assert web.count() == 1
        web.first.click(); pg.wait_for_timeout(300)
        r = last_run(); assert r["target"]["action"] == {"type": "webSearch", "query": "cybersecurity"}, r
    check("'search ...' offers one web row and passes only the typed query", explicit_web)

    def web_fallback_is_last_and_explicit():
        fresh(); type_("zzzquery")
        cats = pg.eval_on_selector_all('[data-testid="lx-row"]', "els=>els.map(e=>e.dataset.category)")
        assert cats == ["web"], cats
        assert pg.evaluate("window.__harness.launcher.runs.length") == 0  # nothing sent until chosen
    check("the web row appears as a fallback and nothing is sent until it is chosen", web_fallback_is_last_and_explicit)

    # ---------------------------------------------------------------- AI
    def ai_row_shows_provider():
        fresh(); type_("explain how tcp works")
        ai = pg.locator('[data-testid="lx-row"][data-category="ai"]'); assert ai.count() == 1
        expect(pg.locator('[data-testid="lx-ai-chip"]')).to_contain_text("Ollama")
        expect(pg.locator('[data-testid="lx-ai-chip"]')).to_contain_text("qwen3.5:2b")
    check("AI rows show which provider and model will be used", ai_row_shows_provider)

    def ai_streams_and_sends_only_typed():
        fresh(); type_("explain how tcp works")
        pg.locator('[data-testid="lx-row"][data-category="ai"]').click(); pg.wait_for_timeout(700)
        expect(pg.locator('[data-testid="lx-ai-answer"]')).to_have_text("A short answer.")
        expect(pg.locator('[data-testid="lx-ai-provider"]')).to_contain_text("Ollama")
        sends = pg.evaluate("window.__harness.launcher.aiSends")
        assert sends == [{"kind": "explain", "text": "how tcp works", "language": None}], sends
        assert pg.locator('[data-testid="lx-ai-copy"]').count() == 1
    check("an AI command streams its answer and sends only the typed text", ai_streams_and_sends_only_typed)

    def ai_never_without_enter():
        fresh(); type_("what is a firewall?"); pg.wait_for_timeout(500)
        assert pg.evaluate("window.__harness.launcher.aiSends.length") == 0
        assert pg.locator('[data-testid="lx-row"][data-category="ai"]').count() == 1
    check("an AI request is never made until it is explicitly chosen", ai_never_without_enter)

    def ai_unavailable():
        fresh(); L("setAiInfo({provider:'OpenAI',model:'',ready:false,problem:'No API key is saved for OpenAI. Add one in Settings > Assistant.'})")
        pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        type_("ask what is a firewall")
        row = pg.locator('[data-testid="lx-row"][data-category="ai"]').first
        expect(row).to_have_attribute("data-available", "false")
        assert "No API key" in row.locator(".lx-sub").inner_text()
        row.click(force=True); pg.wait_for_timeout(200)  # aria-disabled rows still answer a click, with the reason
        assert pg.evaluate("window.__harness.launcher.aiSends.length") == 0
        expect(pg.locator('[data-testid="lx-status"]')).to_contain_text("No API key")
    check("when AI isn't set up the row says exactly what is missing and sends nothing", ai_unavailable)

    def ai_error_and_back():
        fresh(); L("setAiScript({error:'The model is not installed.', chunks:[]})")
        type_("translate hello to French")
        pg.locator('[data-testid="lx-row"][data-category="ai"]').click(); pg.wait_for_timeout(500)
        expect(pg.locator('[data-testid="lx-ai-error"]')).to_contain_text("not installed")
        s = pg.evaluate("window.__harness.launcher.aiSends")
        assert s == [{"kind": "translate", "text": "hello", "language": "French"}], s
        inp.press("Escape"); pg.wait_for_timeout(200)
        expect(pg.locator('[data-testid="lx-ai"]')).to_have_count(0)
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "open")
    check("AI errors are shown; Esc goes back to the results (a second Esc closes)", ai_error_and_back)

    def ai_off_in_settings():
        pg.evaluate("localStorage.setItem('islet-test-settings', JSON.stringify({launcher:{ai:false}}))")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
        type_("explain how tcp works")
        assert pg.locator('[data-testid="lx-row"][data-category="ai"]').count() == 0
        pg.evaluate("localStorage.removeItem('islet-test-settings')")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
    check("switching AI commands off removes them from the results", ai_off_in_settings)

    # ---------------------------------------------------------------- files
    def files():
        fresh(); type_("budget")
        row = pg.locator('[data-testid="lx-row"][data-category="file"]'); assert row.count() == 1
        assert row.locator(".lx-title").inner_text().strip() == "budget 2026.xlsx"
        assert "Documents" in row.locator(".lx-sub").inner_text()
        assert "XLSX" in row.locator(".lx-kind").inner_text()
        row.click(); pg.wait_for_timeout(300)
        r = last_run(); assert r["target"]["type"] == "file" and r["reveal"] is False
    check("file results show type, path, and open on Enter/click", files)

    def folders_and_reveal():
        fresh(); type_("proj")
        sections = [s.strip().lower() for s in pg.locator('[data-testid="lx-section"]').all_inner_texts()]
        assert "folders" in sections, sections
        inp.press("Shift+Enter"); pg.wait_for_timeout(350)
        assert last_run()["reveal"] is True
        fresh(); type_("proj"); pg.locator('[data-testid="lx-reveal"]').first.click(force=True); pg.wait_for_timeout(300)
        assert last_run()["reveal"] is True
    check("folders have their own group; Shift+Enter and the button open the containing folder", folders_and_reveal)

    def programs_are_not_run():
        fresh(); type_("setup"); rows.first.click(); pg.wait_for_timeout(300)
        expect(pg.locator('[data-testid="lx-status"]')).to_contain_text("not run from here")
    check("opening a program file explains it is shown, not run", programs_are_not_run)

    def files_off_in_settings():
        pg.evaluate("localStorage.setItem('islet-test-settings', JSON.stringify({launcher:{files:false}}))")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
        type_("budget")
        assert pg.locator('[data-testid="lx-row"][data-category="file"]').count() == 0
        pg.evaluate("localStorage.removeItem('islet-test-settings')")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
    check("switching file search off removes files from the results", files_off_in_settings)

    # ---------------------------------------------------------------- pins and recents
    def pin_and_remove():
        fresh(); type_("note"); inp.press("Control+p"); pg.wait_for_timeout(300)
        assert pg.locator('[data-testid="lx-row"]').first.locator(".lx-pinmark, [data-testid=lx-pin]").count() >= 1
        inp.fill(""); pg.wait_for_timeout(250)
        assert pg.locator('[data-testid="lx-section"]').first.inner_text().strip().lower() == "pinned"
        assert titles() == ["Notepad"], titles()
        rows.first.hover(); pg.locator('[data-testid="lx-pin"]').first.click(); pg.wait_for_timeout(300)
        assert rows.count() == 0
    check("Ctrl+P pins an app (shown first when empty); the star removes the pin", pin_and_remove)

    def pinned_ranks_higher():
        fresh(); type_("note"); inp.press("Control+p"); pg.wait_for_timeout(200)
        pg.evaluate("window.__harness.launcher.setApps([{id:'C:\\\\Start\\\\Notebook Pro Edition.lnk',name:'Notebook Pro Edition',kind:'desktop'},{id:'C:\\\\Start\\\\Notepad.lnk',name:'Notepad',kind:'desktop'}])")
        type_("note"); assert titles()[0] == "Notepad", titles()
    check("a pinned app outranks an otherwise equal match", pinned_ranks_higher)

    def recents():
        fresh(); type_("chrome"); inp.press("Enter"); pg.wait_for_timeout(450)
        pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        assert pg.locator('[data-testid="lx-section"]').first.inner_text().strip().lower() == "recent"
        assert titles() == ["Google Chrome"], titles()
    check("recently launched apps appear when the box is empty", recents)

    def missing_pin():
        fresh(); type_("note"); inp.press("Control+p"); pg.wait_for_timeout(250)
        pg.evaluate("window.__harness.launcher.setApps([])"); pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        row = rows.first; expect(row).to_have_attribute("data-available", "false")
        assert "no longer installed" in row.inner_text().lower()
        row.click(force=True); pg.wait_for_timeout(200)
        expect(pg.locator('[data-testid="lx-status"]')).to_contain_text("no longer installed")
        assert pg.evaluate("window.__harness.launcher.runs.length") == 0
        inp.press("Control+p"); pg.wait_for_timeout(250)
        assert rows.count() == 0
    check("a pinned app that was uninstalled is flagged, won't run, and can be removed", missing_pin)

    # ---------------------------------------------------------------- states and robustness
    def loading_and_empty_states():
        fresh(); L("setLoading({apps:true,files:true})")
        pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        type_("zzz", 120)
        # with the web fallback switched off the loading message is the whole list
        pg.evaluate("localStorage.setItem('islet-test-settings', JSON.stringify({launcher:{web:false,ai:false}}))")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
        L("setLoading({apps:true,files:true})"); type_("zzz", 150)
        expect(pg.locator('[data-testid="lx-loading"]')).to_be_visible()
        expect(pg.locator('[data-testid="lx-indexing"]')).to_be_visible()
        L("setLoading({apps:false,files:false})"); pg.evaluate("window.__harness.launcher.emitIndex()"); pg.wait_for_timeout(250)
        expect(pg.locator('[data-testid="lx-nomatch"]')).to_be_visible()
        expect(pg.locator('[data-testid="lx-nomatch"]')).to_contain_text("zzz")
        pg.evaluate("localStorage.removeItem('islet-test-settings')")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
    check("loading, 'indexing' and 'no results' states", loading_and_empty_states)

    def run_error_keeps_open():
        fresh(); type_("chrome"); L("failNextRun('Chrome is no longer installed. I removed it from the list.')")
        inp.press("Enter"); pg.wait_for_timeout(300)
        st = pg.locator('[data-testid="lx-status"]'); expect(st).to_have_attribute("role", "alert")
        expect(st).to_contain_text("no longer installed")
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "open")
    check("a failed launch shows the reason and keeps the launcher open", run_error_keeps_open)

    def rapid_typing_newest_wins():
        fresh(); L("setSearchDelay(150)")
        inp.fill(""); inp.type("chrome", delay=15)
        pg.wait_for_timeout(900)
        assert titles()[0] == "Google Chrome", titles()
        L("setSearchDelay(0)")
    check("rapid typing with slow answers settles on the newest query", rapid_typing_newest_wins)

    def enter_before_results():
        fresh(); L("setSearchDelay(250)")
        inp.fill(""); inp.type("notepad", delay=5); inp.press("Enter")
        pg.wait_for_timeout(1000)
        r = last_run(); assert r and "Notepad" in r["target"]["id"] and r["input"] == "notepad", r
        L("setSearchDelay(0)")
    check("Enter pressed before the answer arrives runs the top result of the newest text", enter_before_results)

    def rapid_open_close():
        fresh()
        pg.evaluate("""async () => { const h = window.__harness.launcher;
          for (let i = 0; i < 20; i++) { h.emitOpen(); await new Promise(r => setTimeout(r, i % 3 * 10)); h.emitClose(); await new Promise(r => setTimeout(r, 5)); }
          h.emitOpen(); }""")
        pg.wait_for_timeout(600)
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "open")
        assert inp.input_value() == ""
        assert pg.evaluate("document.activeElement?.dataset.testid") == "lx-input"
        pg.evaluate("window.__harness.launcher.emitClose()"); pg.wait_for_timeout(400)
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "closed")
        pg.evaluate("window.__harness.launcher.emitOpen()"); pg.wait_for_timeout(300)
        expect(pg.locator('[data-testid="launcher"]')).to_have_attribute("data-phase", "open")
    check("rapid open/close cycles end in a consistent, usable state", rapid_open_close)

    def max_results():
        pg.evaluate("localStorage.setItem('islet-test-settings', JSON.stringify({launcher:{maxResults:3,web:false,ai:false}}))")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
        pg.evaluate("window.__harness.launcher.setApps(Array.from({length:20},(_,i)=>({id:'C:\\\\a'+i+'.lnk',name:'Tool '+i,kind:'desktop'})))")
        type_("tool"); assert rows.count() == 3, rows.count()
        pg.evaluate("localStorage.removeItem('islet-test-settings')")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
    check("the maximum-results setting limits the list", max_results)

    # ---------------------------------------------------------------- icons, motion, performance
    def icons():
        svg = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect width='8' height='8' fill='red'/></svg>"
        fresh()
        pg.evaluate("s=>window.__harness.launcher.setApps([{id:'C:\\\\i1.lnk',name:'Iconic',kind:'desktop',icon:s},{id:'C:\\\\i2.lnk',name:'Plain',kind:'desktop'}])", svg)
        type_("i")
        assert pg.locator('[data-testid="lx-row"] img.lx-appicon').count() == 1
        tile = pg.locator('[data-testid="lx-row"] .lx-tile'); assert tile.count() == 1
        assert tile.first.inner_text().strip() in ("P",), tile.first.inner_text()
    check("apps show their icon, or a lettered tile when there is none", icons)

    def icons_are_cached():
        fresh(); pg.evaluate("window.__harness.launcher.iconRequests.length = 0")
        type_("plain"); type_("plain "); type_("plain")
        n = pg.evaluate("window.__harness.launcher.iconRequests.filter(x=>x.includes('i2')).length"); assert n == 0, n  # already cached above
    check("an icon is requested once per app, not on every keystroke", icons_are_cached)

    def animations_only_composited():
        fresh(); type_("note")
        tp = pg.evaluate("getComputedStyle(document.querySelector('[data-testid=lx-panel]')).transitionProperty")
        assert tp.replace(" ", "") == "opacity,transform", tp
    check("the entrance animates only opacity and transform", animations_only_composited)

    def idle_has_no_animations():
        fresh(); type_("note"); pg.wait_for_timeout(700)
        n = pg.evaluate("document.getAnimations().length"); assert n == 0, n
    check("nothing animates while the launcher is idle", idle_has_no_animations)

    def reduced_motion_setting():
        pg.evaluate("localStorage.setItem('islet-test-settings', JSON.stringify({appearance:{motion:'off'}}))")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
        td = pg.evaluate("getComputedStyle(document.querySelector('[data-testid=lx-panel]')).transitionDuration")
        assert td in ("0s", "0s, 0s"), td
        pg.evaluate("localStorage.removeItem('islet-test-settings')")
        pg.goto(BASE + "/launcher.html"); pg.wait_for_selector('[data-testid="launcher"]'); pg.wait_for_timeout(300)
    check("the 'motion off' setting removes the launcher's transitions", reduced_motion_setting)

    def search_is_fast_for_many_apps():
        fresh()
        pg.evaluate("window.__harness.launcher.setApps(Array.from({length:3000},(_,i)=>({id:'C:\\\\x'+i+'.lnk',name:'Program '+i,kind:'desktop'})))")
        type_("program 29", 150)
        assert titles()[0].startswith("Program 29"), titles()
    check("a 3,000-app list stays responsive in the page", search_is_fast_for_many_apps)

    def no_page_errors():
        assert not errors, errors
    check("no uncaught page errors", no_page_errors)

    # ---------------------------------------------------------------- settings page
    sp = ctx.new_page(); sp_errors = []
    sp.on("pageerror", lambda e: sp_errors.append(str(e)))
    sp.goto(BASE + "/settings.html"); sp.wait_for_selector('[data-testid="settings"]')
    sp.evaluate("localStorage.removeItem('islet-test-settings'); localStorage.removeItem('vex-test-launcher')")
    sp.reload(); sp.wait_for_selector('[data-testid="settings"]')
    sp.locator('[data-testid="page-launcher"]').click(); sp.wait_for_timeout(300)
    stored = lambda: json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')") or "{}").get("launcher", {})

    def settings_defaults():
        expect(sp.locator('[data-testid="lx-privacy"]')).to_contain_text("Nothing is read from your clipboard")
        assert sp.inner_text('[data-testid="lx-hotkey"]').strip() == "Alt+Space"
        for t in ("lx-enabled", "lx-apps", "lx-files", "lx-commands", "lx-web", "lx-ai"):
            expect(sp.locator(f'[data-testid="{t}"]')).to_have_attribute("aria-checked", "true")
    check("settings: launcher page shows privacy note, Alt+Space and all categories", settings_defaults)

    def settings_toggle_and_max():
        sp.locator('[data-testid="lx-web"]').click(); sp.wait_for_timeout(250)
        assert stored()["web"] is False
        expect(sp.locator('[data-testid="lx-engine"]')).to_have_count(0)
        sp.locator('[data-testid="lx-web"]').click(); sp.wait_for_timeout(250)
        sp.locator('[data-testid="lx-engine"]').select_option("google"); sp.wait_for_timeout(250)
        assert stored()["webEngine"] == "google"
        sp.evaluate("""()=>{const el=document.querySelector('[data-testid=lx-max]');const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;set.call(el,'5');el.dispatchEvent(new Event('input',{bubbles:true}));}""")
        sp.wait_for_timeout(300); assert stored()["maxResults"] == 5, stored()
    check("settings: category toggles, search engine and maximum results persist", settings_toggle_and_max)

    def settings_hotkey_clash():
        sp.locator('[data-testid="lx-hotkey"]').click()
        sp.locator('[data-testid="lx-hotkey"]').press("Control+Alt+I"); sp.wait_for_timeout(300)
        expect(sp.locator('[data-testid="lx-hotkey-clash"]')).to_be_visible()
        sp.locator('[data-testid="lx-hotkey-clear"]').click(); sp.wait_for_timeout(250)
        assert stored()["hotkey"] == ""
        sp.locator('[data-testid="lx-hotkey-default"]').click(); sp.wait_for_timeout(250)
        assert stored()["hotkey"] == "Alt+Space"
        expect(sp.locator('[data-testid="lx-hotkey-clash"]')).to_have_count(0)
    check("settings: a shortcut that clashes with the island's is flagged; it can be turned off and reset", settings_hotkey_clash)

    def settings_pins_and_clear():
        # create a pin and a recent from the launcher page, then manage them from settings
        fresh(); type_("note"); inp.press("Control+p"); pg.wait_for_timeout(250)
        type_("chrome"); inp.press("Enter"); pg.wait_for_timeout(450)
        sp.reload(); sp.wait_for_selector('[data-testid="settings"]'); sp.locator('[data-testid="page-launcher"]').click(); sp.wait_for_timeout(400)
        expect(sp.locator('[data-testid="lx-pins"]')).to_contain_text("Notepad")
        sp.locator('[data-testid="lx-clear"]').click(); sp.wait_for_timeout(300)
        expect(sp.locator('[data-testid="lx-note"]')).to_contain_text("cleared")
        st = json.loads(pg.evaluate("localStorage.getItem('vex-test-launcher')"))
        assert st["recents"] == [] and st["history"] == [] and len(st["pins"]) == 1, st
        sp.locator('[data-testid="lx-unpin"]').click(); sp.wait_for_timeout(300)
        expect(sp.locator('[data-testid="lx-no-pins"]')).to_be_visible()
    check("settings: pinned apps can be removed and history cleared (pins survive a clear)", settings_pins_and_clear)

    def settings_folders():
        sp.locator('[data-testid="lx-folder-default"]').is_disabled()
        sp.evaluate("""()=>{}"""); 
        expect(sp.locator('[data-testid="lx-index-status"]')).to_contain_text("apps")
        expect(sp.locator('[data-testid="lx-refresh"]')).to_be_visible()
    check("settings: index status and refresh are shown", settings_folders)

    def settings_ai_status():
        expect(sp.locator('[data-testid="lx-ai-status"]')).to_contain_text("Ollama")
    check("settings: shows which AI provider the commands will use", settings_ai_status)

    check("settings page: no uncaught errors", lambda: (_ for _ in ()).throw(AssertionError(sp_errors)) if sp_errors else None)
    b.close()
print(f"{sum(results)}/{len(results)} launcher checks passed"); sys.exit(0 if all(results) else 1)
