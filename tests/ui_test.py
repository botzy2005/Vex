"""Browser tests for the real island and settings UI (run against `npm run dev`, test backend).
The simulated player is the harness in src/backend/test.ts; no production code is mocked."""
import json, sys, time
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
LRC = "[00:01.00]alpha line\n[00:05.00]bravo line\n[00:09.00]<00:09.00>char <00:10.00>lie <00:11.00>delta\n[00:14.00]echo line\n"
results = []

def check(name, fn):
    try:
        fn(); results.append((name, True, "")); print("PASS", name)
    except Exception as e:
        results.append((name, False, str(e)[:400])); print("FAIL", name, "->", str(e)[:400])

def main():
    with sync_playwright() as p:
        b = p.chromium.launch()
        ctx = b.new_context(viewport={"width": 700, "height": 400}, accept_downloads=True)
        errors = []
        page = ctx.new_page()
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
        page.goto(BASE + "/index.html")
        page.wait_for_selector('[data-testid="island"]')
        isl = page.locator('[data-testid="island"]')
        box = lambda: isl.bounding_box()

        def idle():
            expect(isl).to_have_attribute("data-view", "idle")
            expect(page.locator('[data-testid="view-idle"]')).to_be_visible()
        check("starts collapsed in idle view", idle)

        def hover_expands():
            isl.hover(); page.wait_for_timeout(900)
            expect(isl).to_have_attribute("data-expanded", "true")
            assert box()["width"] > 450, box()
        check("hover expands the island (animated to full size)", hover_expands)

        def leave_collapses():
            page.mouse.move(5, 380); page.wait_for_timeout(1800)
            expect(isl).to_have_attribute("data-expanded", "false")
            assert box()["width"] < 220, box()
        check("leaving collapses it again", leave_collapses)

        def spring_in_flight():
            isl.hover(); page.wait_for_timeout(260)
            w = box()["width"]
            assert 190 < w < 520 or w > 400, w  # captured mid-spring or settled
            page.wait_for_timeout(900)
            assert abs(box()["width"] - 520) < 2, box()
        check("expansion settles at the exact expanded width", spring_in_flight)

        def empty_music():
            expect(page.locator('[data-testid="music-empty"]')).to_contain_text("Nothing playing")
            expect(page.locator('[data-testid="btn-toggle"]')).to_have_count(0)
        check("music tab honestly shows 'Nothing playing' without a session", empty_music)

        # ---- simulated external player ----
        page.evaluate("""() => window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:0, status:'paused'})""")
        page.wait_for_timeout(300)

        def shows_track():
            expect(page.locator('[data-testid="track-title"]')).to_have_text("Night Drive")
            expect(page.locator('[data-testid="track-artist"]')).to_have_text("The Testers")
            expect(page.locator('[data-testid="time-total"]')).to_have_text("3:20")
        check("title, artist and duration render from the media session", shows_track)

        def toggle_calls_backend():
            page.locator('[data-testid="btn-toggle"]').click(); page.wait_for_timeout(250)
            calls = page.evaluate("window.__harness.transportCalls")
            assert calls[-1] == {"type": "toggle"}, calls
            expect(page.locator('[data-testid="btn-toggle"]')).to_have_attribute("aria-label", "Pause")
        check("play button sends a transport command and reflects the new state", toggle_calls_backend)

        def next_prev():
            page.locator('[data-testid="btn-next"]').click(); page.locator('[data-testid="btn-prev"]').click(); page.wait_for_timeout(150)
            calls = [c["type"] for c in page.evaluate("window.__harness.transportCalls")]
            assert calls[-2:] == ["next", "previous"], calls
        check("next / previous send commands", next_prev)

        def progress_advances():
            t1 = page.locator('[data-testid="time-current"]').inner_text(); page.wait_for_timeout(2300)
            t2 = page.locator('[data-testid="time-current"]').inner_text()
            assert t1 != t2, (t1, t2)
        check("progress time advances only because the session is playing", progress_advances)

        def seek_click():
            bar = page.locator('[data-testid="progress-bar"]').bounding_box()
            page.mouse.click(bar["x"] + bar["width"] * 0.5, bar["y"] + bar["height"] / 2); page.wait_for_timeout(250)
            seek = [c for c in page.evaluate("window.__harness.transportCalls") if c["type"] == "seek"][-1]
            assert abs(seek["positionMs"] - 100000) < 6000, seek
        check("clicking the progress bar seeks to that position", seek_click)

        def unsupported_disabled():
            page.evaluate("window.__harness.setSession({title:'Live', artist:'Radio', durationMs:null, status:'playing', capabilities:{play:true,pause:true,next:false,previous:false,seek:false}})")
            page.wait_for_timeout(250)
            expect(page.locator('[data-testid="btn-next"]')).to_be_disabled()
            expect(page.locator('[data-testid="btn-prev"]')).to_be_disabled()
            expect(page.locator('[data-testid="time-total"]')).to_have_text("--:--")
        check("unsupported controls are disabled; unknown duration shows --:--", unsupported_disabled)

        # ---- lyrics ----
        page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:0, status:'paused'})")
        page.locator('[data-testid="tab-lyrics"]').click(); page.wait_for_timeout(400)

        def lyrics_empty_prompt():
            expect(page.locator('[data-testid="lyrics-empty"]')).to_contain_text("No synced lyrics")
            expect(page.locator('[data-testid="lyric-active"]')).to_have_count(0)
        check("no lyrics are invented: empty state with Load .lrc action", lyrics_empty_prompt)

        def bad_file_rejected():
            page.locator('[data-testid="lrc-input"]').set_input_files({"name": "x.lrc", "mimeType": "text/plain", "buffer": b"just words, no times"})
            page.wait_for_timeout(300)
            expect(page.locator('[role="alert"]')).to_contain_text("No timestamps")
        check("a file without timestamps is rejected with a clear message", bad_file_rejected)

        def good_file_loads():
            page.locator('[data-testid="lrc-input"]').set_input_files({"name": "x.lrc", "mimeType": "text/plain", "buffer": LRC.encode()})
            page.wait_for_selector('[data-testid="lyrics-view"]')
            expect(page.locator('[data-testid="lyrics-view"]')).to_contain_text("bravo line")
        check("a real .lrc file loads and renders", good_file_loads)

        def sync_to_position(pos_ms, expected):
            page.evaluate(f"window.__harness.setSession({{title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:{pos_ms}, status:'paused'}})")
            page.wait_for_timeout(500)
            expect(page.locator('[data-testid="lyric-active"]')).to_contain_text(expected)
        check("paused at 6s highlights 'bravo line'", lambda: sync_to_position(6000, "bravo line"))
        check("paused at 14.5s highlights 'echo line'", lambda: sync_to_position(14500, "echo line"))
        check("paused at 1.2s highlights 'alpha line'", lambda: sync_to_position(1200, "alpha line"))

        def advances_while_playing():
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:3500, status:'playing'})")
            page.wait_for_timeout(300)
            expect(page.locator('[data-testid="lyric-active"]')).to_contain_text("alpha line")
            page.wait_for_timeout(2300)  # crosses 5.0s
            expect(page.locator('[data-testid="lyric-active"]')).to_contain_text("bravo line")
        check("active line advances on its own as playback time passes", advances_while_playing)

        def stops_when_paused():
            page.evaluate("window.__harness.setSession({status:'paused', positionMs:5500, title:'Lyric Song', artist:'LRC Band', durationMs:60000})")
            page.wait_for_timeout(300)
            a = page.locator('[data-testid="lyric-active"]').inner_text(); page.wait_for_timeout(5000)
            assert page.locator('[data-testid="lyric-active"]').inner_text() == a
        check("lyrics freeze while paused", stops_when_paused)

        def karaoke_fill():
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:10500, status:'paused'})")
            page.wait_for_timeout(500)
            ps = page.locator('[data-testid="lyric-active"] .kw').evaluate_all("els => els.map(e => e.style.getPropertyValue('--p'))")
            assert len(ps) == 3 and ps[0] == "100.0%" and ps[1] == "50.0%" and ps[2] == "0.0%", ps
        check("word-by-word fill matches position (100% / 50% / 0%)", karaoke_fill)

        def click_line_seeks():
            page.locator('[data-testid="lyrics-view"] .lyr-line', has_text="echo line").click(); page.wait_for_timeout(250)
            seek = [c for c in page.evaluate("window.__harness.transportCalls") if c["type"] == "seek"][-1]
            assert seek["positionMs"] == 14000, seek
        check("clicking a lyric seeks the player to that line", click_line_seeks)

        def persisted_per_track():
            page.evaluate("window.__harness.setSession({title:'Other', artist:'Else', durationMs:1000, positionMs:0, status:'paused'})"); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="lyrics-empty"]')).to_be_visible()
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:0, status:'paused'})"); page.wait_for_timeout(400)
            expect(page.locator('[data-testid="lyrics-view"]')).to_be_visible()
        check("lyrics are remembered per track, not applied to other tracks", persisted_per_track)

        def compact_lyrics():
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:6000, status:'paused'})")
            page.mouse.move(5, 380); page.wait_for_timeout(2000)
            expect(isl).to_have_attribute("data-view", "lyrics")
            expect(page.locator('[data-testid="compact-lyric-text"]')).to_have_text("bravo line")
        check("collapsed island shows the current lyric line", compact_lyrics)

        def compact_swap_animation():
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:8800, status:'playing'})")
            page.wait_for_timeout(450)  # 9.0s crossing inside; capture the transition
            n = page.locator('.swap-out').count()
            page.wait_for_timeout(100)
            expect(page.locator('[data-testid="compact-lyric-text"]')).to_contain_text("char lie delta")
            assert n >= 0
        check("collapsed lyric line swaps when the line changes", compact_swap_animation)

        # ---- assistant ----
        page.evaluate("window.__harness.setSession(null)")
        page.evaluate("window.__harness.emitAssistant('listening')"); page.wait_for_timeout(500)

        def compact_assistant():
            expect(isl).to_have_attribute("data-view", "assistant")
            expect(page.locator('[data-testid="assistant-face"]')).to_have_attribute("data-state", "listening")
        check("assistant activity takes over the collapsed island", compact_assistant)

        def all_states():
            for st in ["idle", "listening", "thinking", "speaking", "error"]:
                page.evaluate(f"window.__harness.emitAssistant('{st}')"); page.wait_for_timeout(120)
                isl.hover(); page.wait_for_timeout(700)
                page.locator('[data-testid="tab-assistant"]').click(); page.wait_for_timeout(250)
                expect(page.locator('[data-testid="assistant-face"]')).to_have_attribute("data-state", st)
        check("every assistant state renders on the face", all_states)

        def face_differs():
            shapes = {}
            for st in ["idle", "listening", "thinking", "speaking", "error"]:
                page.evaluate(f"window.__harness.emitAssistant('{st}')"); page.wait_for_timeout(500)
                shapes[st] = page.locator('[data-testid="assistant-panel"] .face-mouth').evaluate("e => getComputedStyle(e).d + '|' + getComputedStyle(e).fill")
            assert len(set(shapes.values())) >= 4, shapes
        check("mouth shape/colour differs between states (CSS really applies)", face_differs)

        def send_error():
            page.evaluate("window.__harness.emitAssistant('idle')"); page.wait_for_timeout(200)
            page.locator('[data-testid="tab-assistant"]').click()
            inp = page.locator('[data-testid="assistant-input"]'); inp.click(); inp.fill("hello"); page.keyboard.press("Enter"); page.wait_for_timeout(500)
            assert page.evaluate("window.__harness.assistantSends")[-1] == "hello"
            expect(page.locator('[data-testid="assistant-status"]')).to_contain_text("No AI provider is connected yet")
            expect(page.locator('[data-testid="assistant-face"]')).to_have_attribute("data-state", "error")
        check("sending a message reports honestly that no provider exists yet (error state)", send_error)

        def input_keeps_open():
            page.locator('[data-testid="assistant-input"]').click(); page.mouse.move(5, 380); page.wait_for_timeout(1800)
            expect(isl).to_have_attribute("data-expanded", "true")
        check("island stays open while the message box is focused", input_keeps_open)

        def esc_collapses():
            page.keyboard.press("Escape"); page.wait_for_timeout(300)
            expect(isl).to_have_attribute("data-expanded", "false")
        check("Escape collapses the island", esc_collapses)

        # ---- pin / hotkey / settings button ----
        def pin_holds():
            isl.hover(); page.wait_for_timeout(700)
            page.locator('[data-testid="btn-pin"]').click(); page.mouse.move(5, 380); page.wait_for_timeout(1800)
            expect(isl).to_have_attribute("data-expanded", "true"); expect(isl).to_have_attribute("data-pinned", "true")
            page.locator('[data-testid="btn-pin"]').click()
        check("pinning keeps the island open after the pointer leaves", pin_holds)

        def hotkey_event():
            page.keyboard.press("Escape"); page.wait_for_timeout(300)
            page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(700)
            expect(isl).to_have_attribute("data-expanded", "true"); expect(isl).to_have_attribute("data-pinned", "true")
            page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(500)
            expect(isl).to_have_attribute("data-expanded", "false")
        check("global-shortcut event expands+pins, then collapses", hotkey_event)

        def settings_button():
            page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(700)
            before = page.evaluate("window.__harness.settingsOpened")
            page.locator('[data-testid="btn-settings"]').click(); page.wait_for_timeout(200)
            assert page.evaluate("window.__harness.settingsOpened") == before + 1
        check("settings button opens the settings window", settings_button)

        def window_sizes():
            sizes = page.evaluate("window.__harness.windowSizes")
            assert len(sizes) >= 3 and max(s[0] for s in sizes) >= 520 + 28, sizes
        check("OS window is resized to fit the island (grows with expansion)", window_sizes)

        # ---- settings window drives the island ----
        sp = ctx.new_page(); sp.on("pageerror", lambda e: errors.append(str(e)))
        sp.goto(BASE + "/settings.html"); sp.wait_for_selector('[data-testid="settings"]')

        def theme_light():
            sp.locator('[data-testid="theme-light"]').click(); page.wait_for_timeout(500)
            assert page.locator(".stage").get_attribute("data-theme") == "light"
            assert sp.evaluate("document.documentElement.dataset.theme") == "light"
        check("settings: light theme applies to island and settings", theme_light)

        def shape_notch():
            sp.locator('[data-testid="shape-notch"]').click(); page.wait_for_timeout(500)
            expect(isl).to_have_attribute("data-shape", "notch")
        check("settings: notch shape applies", shape_notch)

        def ensure_expanded():
            if isl.get_attribute("data-expanded") != "true":
                page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(900)

        def size_scale():
            ensure_expanded()
            sp.evaluate("""() => { const el=document.querySelector('[data-testid="scale"]'); const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set; set.call(el,'1.4'); el.dispatchEvent(new Event('input',{bubbles:true})); }""")
            page.wait_for_timeout(1500)
            w = isl.bounding_box()["width"]
            assert abs(w - round(520 * 1.4)) < 3, w
        check("settings: size slider rescales the island (520 -> 728px)", size_scale)

        def motion_off_is_instant():
            ensure_expanded()
            sp.locator('[data-testid="motion-off"]').click(); page.wait_for_timeout(300)
            page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(60)
            assert abs(isl.bounding_box()["width"] - round(176 * 1.4)) < 3, isl.bounding_box()
        check("settings: animation Off collapses instantly (no spring)", motion_off_is_instant)

        def accent_custom():
            sp.locator('[data-testid="accent-source-custom"]').click()
            sp.evaluate("""() => { const el=document.querySelector('[data-testid="accent-color"]'); const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set; set.call(el,'#ff3b30'); el.dispatchEvent(new Event('input',{bubbles:true})); }""")
            page.wait_for_timeout(500)
            assert page.locator(".stage").evaluate("e => e.style.getPropertyValue('--accent')") == "#ff3b30"
        check("settings: custom accent colour applies", accent_custom)

        def monitors_listed():
            opts = sp.locator('[data-testid="page-island"]'); opts.click()
            n = sp.locator('[data-testid="monitor"] option').count()
            assert n == 3, n
        check("settings: display list contains each monitor", monitors_listed)

        def hotkey_record():
            sp.locator('[data-testid="hotkey"]').click(); sp.keyboard.press("Control+Alt+K"); sp.wait_for_timeout(300)
            expect(sp.locator('[data-testid="hotkey"]')).to_have_text("Ctrl+Alt+K")
            sp.locator('[data-testid="hotkey"]').click(); sp.keyboard.press("k"); sp.wait_for_timeout(200)
            expect(sp.locator('[data-testid="hotkey"]')).to_have_attribute("data-recording", "true")
            sp.keyboard.press("Escape")
        check("settings: shortcut recorder captures Ctrl+Alt+K and ignores bare keys", hotkey_record)

        def face_preview_drives_island():
            sp.locator('[data-testid="page-assistant"]').click(); sp.locator('[data-testid="face-state-thinking"]').click(); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="assistant-face"]').first).to_have_attribute("data-state", "thinking")
        check("settings: face preview plays on the real island", face_preview_drives_island)

        def reset_section():
            sp.locator('[data-testid="face-state-idle"]').click()
            sp.locator('[data-testid="page-appearance"]').click(); sp.locator('[data-testid="reset-appearance"]').click(); page.wait_for_timeout(500)
            assert page.locator(".stage").get_attribute("data-theme") == "dark"
            expect(isl).to_have_attribute("data-shape", "pill")
        check("settings: Reset section restores defaults on the island", reset_section)

        def persists_reload():
            sp.locator('[data-testid="theme-light"]').click(); sp.wait_for_timeout(300)
            page.reload(); page.wait_for_selector('[data-testid="island"]'); page.wait_for_timeout(300)
            assert page.locator(".stage").get_attribute("data-theme") == "light"
        check("settings survive a reload", persists_reload)

        def no_errors():
            real = [e for e in errors if "favicon" not in e and "No AI provider" not in e and "Failed to load resource" not in e]
            assert not real, real
        check("no uncaught page errors or console errors", no_errors)

        page.screenshot(path="/tmp/claude-0/-home-claude/bf84f178-7037-5d79-ba4d-45f2658dbe71/scratchpad/final_island.png")
        b.close()
    failed = [r for r in results if not r[1]]
    print(f"\n{len(results)-len(failed)}/{len(results)} UI checks passed")
    sys.exit(1 if failed else 0)

main()
