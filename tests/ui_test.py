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
            assert 190 < w < 540 or w > 400, w  # captured mid-spring or settled
            page.wait_for_timeout(900)
            assert abs(box()["width"] - 540) < 2, box()
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
            expect(page.locator('[data-testid="lyrics-empty"]')).to_contain_text("No synced lyrics found")
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
            page.evaluate("window.__harness.patchSettings({music:{lyricsStyle:'line'}})")
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:10500, status:'paused'})")
            page.wait_for_timeout(500)
            ps = page.locator('[data-testid="lyric-active"] .kw').evaluate_all("els => els.map(e => e.style.getPropertyValue('--p'))")
            assert len(ps) == 3 and ps[0] == "100.0%" and ps[1] == "50.0%" and ps[2] == "0.0%", ps
        check("word-by-word fill matches position (100% / 50% / 0%)", karaoke_fill)
        page.evaluate("window.__harness.patchSettings({music:{lyricsStyle:'letters'}})")

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
            expect(page.locator('[data-testid="compact-lyric"] [data-testid="letter-line"]')).to_have_text("bravo line")
        check("collapsed island shows the current lyric line", compact_lyrics)

        def compact_swap_animation():
            page.evaluate("window.__harness.setSession({title:'Lyric Song', artist:'LRC Band', durationMs:60000, positionMs:8800, status:'playing'})")
            page.wait_for_timeout(450)  # 9.0s crossing inside; capture the transition
            page.wait_for_timeout(100)
            expect(page.locator('[data-testid="compact-lyric"] [data-testid="letter-line"]')).to_have_text("char lie delta")
        check("collapsed lyric line changes when the line changes", compact_swap_animation)

        # ---- letters, automatic lookup, statuses ----
        AUTO = "[00:01.00]first auto line\n[00:05.00]second auto line\n[00:09.00]third auto line\n"
        def levels():
            return page.locator('[data-testid="compact-lyric"] .lf').evaluate_all("els => els.map(e => parseFloat(e.style.getPropertyValue('--l')))")
        def set_pos(pos, status='paused', title='Lyric Song', artist='LRC Band', dur=60000):
            page.evaluate(f"window.__harness.setSession({{title:'{title}', artist:'{artist}', durationMs:{dur}, positionMs:{pos}, status:'{status}'}})")
            page.wait_for_timeout(450)

        def letters_structure():
            set_pos(6000)
            n = page.locator('[data-testid="compact-lyric"] .lt').count()
            assert n == len("bravoline"), n
            ds = page.locator('[data-testid="compact-lyric"] .lt').evaluate_all("els => els.map(e => parseFloat(e.style.getPropertyValue('--d')))")
            assert ds == sorted(ds) and ds[-1] > ds[0] >= 0, ds
        check("each letter is its own element with a rising stagger delay", letters_structure)

        def letters_sweep():
            set_pos(5000); l0 = levels()
            assert all(v == 0 for v in l0), l0
            set_pos(5400); lm = levels()
            assert lm[0] > 0 and lm[-1] == 0 and all(lm[i] >= lm[i+1] for i in range(len(lm)-1)), lm
            set_pos(8500); l1 = levels()
            assert all(v == 1 for v in l1), l1
        check("letters light up left to right with the song position (and freeze when paused)", letters_sweep)

        def letters_follow_words():
            set_pos(10500)  # line 'char lie delta': 'char' fully sung, 'lie' half
            l = levels()
            assert all(v == 1 for v in l[:5]) and 0 < l[5] < 1 and l[-1] == 0, l  # 'char' + 'l' sung, 'i' mid-sweep
        check("with word timing the sweep follows the real words", letters_follow_words)

        def line_style_setting():
            page.evaluate("window.__harness.patchSettings({music:{lyricsStyle:'line'}})"); set_pos(6000)
            expect(page.locator('[data-testid="compact-lyric-text"]')).to_have_text("bravo line")
            assert page.locator('[data-testid="compact-lyric"] .lt').count() == 0
            page.evaluate("window.__harness.patchSettings({music:{lyricsStyle:'letters'}})"); page.wait_for_timeout(300)
        check("'Line' animation setting switches letters off", line_style_setting)

        def auto_lookup():
            page.evaluate("window.__harness.setLyricsReply({synced: %r, plain: null, instrumental: false, source: 'LRCLIB'})" % AUTO)
            set_pos(6000, title='Auto Song', artist='Net Band', dur=30000)
            calls = page.evaluate("window.__harness.lyricsCalls")
            assert any(c["title"] == "Auto Song" and c["artist"] == "Net Band" and c["durationMs"] == 30000 for c in calls), calls
            expect(page.locator('[data-testid="compact-lyric"] [data-testid="letter-line"]')).to_have_text("second auto line")
        check("a new track triggers automatic lookup and its lyrics appear without any file", auto_lookup)

        def local_file_wins():
            # 'Lyric Song' has a user .lrc; the online service must not override it
            page.evaluate("window.__harness.setLyricsReply({synced: %r, plain: null, instrumental: false, source: 'LRCLIB'})" % AUTO)
            set_pos(6000)
            expect(page.locator('[data-testid="compact-lyric"] [data-testid="letter-line"]')).to_have_text("bravo line")
        check("the user's own .lrc takes priority over the online result", local_file_wins)

        isl.hover(); page.wait_for_timeout(800)
        def peek_in_player():
            page.locator('[data-testid="tab-music"]').click(); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="peek-now"]')).to_have_text("bravo line")
            assert abs(box()["height"] - 218) < 2, box()
        check("the player shows the current lyric next to the controls", peek_in_player)
        page.locator('[data-testid="tab-lyrics"]').click(); page.wait_for_timeout(300)

        def lyrics_tab_attribution():
            set_pos(6000, title='Auto Song', artist='Net Band', dur=30000)
            expect(page.locator('[data-testid="lyrics-view"]')).to_contain_text("Lyrics from LRCLIB")
        check("lyrics tab credits LRCLIB for online lyrics", lyrics_tab_attribution)

        def error_then_retry():
            page.evaluate("window.__harness.setLyricsReply({error: 'offline'})")
            set_pos(0, title='Err Song', artist='Err Band', dur=30000)
            expect(page.locator('[data-testid="lyrics-empty"]')).to_contain_text("Couldn't reach the lyrics service")
            page.evaluate("window.__harness.setLyricsReply({synced: %r, plain: null, instrumental: false, source: 'LRCLIB'})" % AUTO)
            page.locator('[data-testid="retry-lyrics"]').click(); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="lyrics-view"]')).to_contain_text("third auto line")
        check("a network failure says so honestly, and Try again recovers", error_then_retry)

        def plain_only():
            page.evaluate("window.__harness.setLyricsReply({synced: null, plain: 'unsynced words here', instrumental: false, source: 'LRCLIB'})")
            set_pos(0, title='Plain Song', artist='Plain Band', dur=30000)
            expect(page.locator('[data-testid="lyrics-plain"]')).to_contain_text("unsynced words here")
            expect(page.locator('[data-testid="lyrics-plain"]')).to_contain_text("without timing")
            assert page.locator('[data-testid="lyric-active"]').count() == 0
        check("unsynced lyrics are shown as static text, never faked as synced", plain_only)

        def instrumental():
            page.evaluate("window.__harness.setLyricsReply({synced: null, plain: null, instrumental: true, source: 'LRCLIB'})")
            set_pos(0, title='Inst Song', artist='Inst Band', dur=30000)
            expect(page.locator('[data-testid="lyrics-empty"]')).to_contain_text("Instrumental")
        check("instrumental tracks are labelled", instrumental)

        def online_off():
            page.evaluate("window.__harness.setLyricsReply({synced: %r, plain: null, instrumental: false, source: 'LRCLIB'})" % AUTO)
            page.evaluate("window.__harness.patchSettings({music:{lyricsOnline:false}})")
            before = len(page.evaluate("window.__harness.lyricsCalls"))
            set_pos(0, title='Private Song', artist='Priv Band', dur=30000)
            expect(page.locator('[data-testid="lyrics-empty"]')).to_contain_text("Online lookup is off")
            assert len(page.evaluate("window.__harness.lyricsCalls")) == before
            page.evaluate("window.__harness.patchSettings({music:{lyricsOnline:true}})")
        check("with online lookup off nothing is requested", online_off)

        set_pos(6000)
        page.mouse.move(5, 380); page.wait_for_timeout(1800)

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

        # ---- real chat (the model behind it is the scripted test double; see src/backend/test.ts) ----
        H = "window.__harness"
        def goto_assistant():
            isl.hover(); page.wait_for_timeout(800)
            if isl.get_attribute("data-expanded") != "true":  # the pointer was already over the island: no hover event fired
                page.mouse.move(5, 380); page.wait_for_timeout(300); isl.hover(); page.wait_for_timeout(900)
            if isl.get_attribute("data-expanded") != "true":
                page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(900)
            page.locator('[data-testid="tab-assistant"]').click(); page.wait_for_timeout(300)
        def say(msg):
            inp = page.locator('[data-testid="assistant-input"]'); inp.click(); inp.fill(msg); page.keyboard.press("Enter")
        def texts(testid):
            return page.locator(f'[data-testid="{testid}"]').all_inner_texts()
        page.evaluate("window.__harness.emitAssistant('idle')"); page.wait_for_timeout(200)
        goto_assistant()

        def not_configured():
            say("hello"); page.wait_for_timeout(400)
            expect(page.locator('[data-testid="chat-error"]')).to_contain_text("Choose a model")
            assert page.locator('[data-testid="msg-user"]').count() == 0, "a refused message must not be stored"
            assert page.locator('[data-testid="assistant-input"]').input_value() == "hello", "the text must be kept"
            expect(page.locator('[data-testid="setup-hint"]')).to_be_visible()
        check("unconfigured assistant refuses honestly, stores nothing and keeps your text", not_configured)

        page.evaluate("window.__harness.patchSettings({ai:{models:{ollama:'llama3.2:latest'}}})"); page.wait_for_timeout(300)
        page.locator('[data-testid="assistant-input"]').fill("")

        def streams():
            page.evaluate("window.__harness.setAiScript({chunks:['Hello',' **world**',' from the model.'], delayMs:450})")
            say("Hi there"); page.wait_for_timeout(150)
            expect(page.locator('[data-testid="typing"]')).to_be_visible()
            page.wait_for_timeout(700)
            first = " ".join(texts("msg-streaming"))
            assert "Hello" in first and "model" not in first, first
            page.wait_for_timeout(1100)
            assert page.locator('[data-testid="msg-streaming"]').count() == 0
            final = texts("msg-assistant")[-1]
            assert "Hello world from the model." in final.replace("\n", " "), final
            assert page.locator('[data-testid="msg-assistant"] strong').inner_text() == "world"
            expect(page.locator('[data-testid="assistant-send"]')).to_be_visible()
        check("reply streams in pieces (typing dots, then growing text) and renders Markdown", streams)

        def face_follows():
            page.evaluate("window.__harness.setAiScript({chunks:['a','b','c','d'], delayMs:700})")
            say("again"); page.wait_for_timeout(200)
            page.locator('[data-testid="assistant-input"]').blur()
            page.mouse.move(5, 380); page.wait_for_timeout(1500)
            expect(isl).to_have_attribute("data-expanded", "false")
            expect(isl).to_have_attribute("data-view", "assistant")  # the collapsed island shows the assistant at work
            expect(page.locator('[data-testid="assistant-face"]').first).to_have_attribute("data-state", "thinking")
            page.wait_for_timeout(2600)
            assert isl.get_attribute("data-view") != "assistant", "back to normal when the reply is done"
            goto_assistant()
        check("while a reply is on its way the collapsed island shows the thinking face, then returns to normal", face_follows)

        def follow_up_context():
            sends = page.evaluate("window.__harness.aiSends")
            assert sends[-2]["conversationId"] is None, "the first message starts a new conversation"
            assert sends[-1]["conversationId"] is not None, "the follow-up carries the conversation id"
            assert page.locator('[data-testid="msg-user"]').count() == 2 and page.locator('[data-testid="msg-assistant"]').count() == 2
        check("a follow-up continues the same conversation", follow_up_context)

        def code_and_lists():
            page.evaluate("window.__harness.setAiScript({chunks:['Steps:\\n\\n1. one\\n2. two\\n\\n```py\\nprint(1)\\n```\\n'], delayMs:50})")
            say("show code"); page.wait_for_timeout(900)
            assert page.locator('[data-testid="msg-assistant"] ol li').count() >= 2
            assert page.locator('[data-testid="msg-assistant"] pre code').last.inner_text() == "print(1)"
            expect(page.locator('[data-testid="copy-code"]').last).to_be_visible()
        check("lists and fenced code blocks render (with a Copy button)", code_and_lists)

        def html_is_inert():
            page.evaluate("window.__harness.setAiScript({chunks:['<img src=x onerror=window.__pwn=1> <script>window.__pwn=2</script>'], delayMs:50})")
            say("xss"); page.wait_for_timeout(700)
            assert page.evaluate("window.__pwn") is None
            assert page.locator('[data-testid="msg-assistant"] img').count() == 0
        check("HTML in a model reply is shown as text, never executed", html_is_inert)

        def stop_button():
            page.evaluate("window.__harness.setAiScript({chunks:['one ','two ','three ','four ','five'], delayMs:400})")
            say("long one"); page.wait_for_timeout(1000)
            page.locator('[data-testid="assistant-stop"]').click(); page.wait_for_timeout(900)
            expect(page.locator('[data-testid="partial-note"]').last).to_be_visible()
            assert "five" not in texts("msg-assistant")[-1]
            expect(page.locator('[data-testid="assistant-send"]')).to_be_visible()
        check("Stop ends the answer, keeps what arrived and marks it as cut short", stop_button)

        def provider_error():
            page.evaluate("window.__harness.setAiScript({chunks:['partial text'], delayMs:50, error:{message:'The provider rejected your API key.', kind:'auth'}})")
            say("will fail"); page.wait_for_timeout(900)
            expect(page.locator('[data-testid="chat-error"]')).to_contain_text("rejected your API key")
            expect(page.locator('[data-testid="chat-error"]')).to_contain_text("Open Settings")
            assert "partial text" in texts("msg-assistant")[-1]
            page.locator('[data-testid="chat-error"] >> text=Dismiss').click()
            assert page.locator('[data-testid="chat-error"]').count() == 0
        check("a failed request shows a clear error, keeps the partial text, and can be dismissed", provider_error)

        # app context: shared only when asked
        page.evaluate("window.__harness.setAiScript({chunks:['ok'], delayMs:30})")
        page.evaluate("window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:62000, status:'playing'})"); page.wait_for_timeout(400)
        def last_send(): return page.evaluate("window.__harness.aiSends")[-1]
        def ctx_only_when_asked():
            say("Tell me a joke"); page.wait_for_timeout(500)
            assert last_send()["context"] is None, last_send()
            say("What song is playing?"); page.wait_for_timeout(500)
            c = last_send()["context"]
            assert c and "Night Drive" in c["text"] and "The Testers" in c["text"] and c["label"] == "Now playing", c
            expect(page.locator('[data-testid="ctx-chip"]').last).to_contain_text("Now playing")
            say("How do I play a song on guitar?"); page.wait_for_timeout(500)
            assert last_send()["context"] is None
        check("song context is attached only to messages that ask about the music, and is shown on the message", ctx_only_when_asked)

        def ctx_real_phrasings():
            # the wording from real use: none of these matched before
            for q in ["What song am I currently listening to?", "What song is this?", "what's playing", "who sings this", "Who is the artist?"]:
                say(q); page.wait_for_timeout(450)
                c = last_send()["context"]
                assert c and "Night Drive" in c["text"] and "The Testers" in c["text"], (q, c)
                assert "lyrics" not in c["label"].lower(), (q, c["label"])  # a song question does not drag the lyrics along
        check("natural questions like 'What song am I currently listening to?' attach the title and artist", ctx_real_phrasings)

        def ctx_unrelated_stays_private():
            for q in ["Explain cloud security in simple terms.", "Write a song about cats", "recommend some songs for studying", "What's playing at the cinema tonight?", "Compare cloud security and network security in a table"]:
                say(q); page.wait_for_timeout(450)
                assert last_send()["context"] is None, (q, last_send()["context"])
        check("unrelated questions (including music-adjacent ones) never carry the music", ctx_unrelated_stays_private)

        def ctx_lyrics_only_when_asked():
            page.evaluate("window.__harness.setLyricsReply({synced:'[00:01.00]first made-up line\\n[00:05.00]second made-up line\\n', plain:null, instrumental:false, source:'LRCLIB'})")
            page.evaluate("window.__harness.setSession({title:'Lyric Track', artist:'LRC Band', durationMs:60000, positionMs:6000, status:'paused'})"); page.wait_for_timeout(900)
            say("What song am I listening to?"); page.wait_for_timeout(450)
            c = last_send()["context"]; assert c and "Lyric Track" in c["text"] and "made-up line" not in c["text"], c
            say("Explain these lyrics"); page.wait_for_timeout(450)
            c = last_send()["context"]; assert c and "first made-up line" in c["text"] and "second made-up line" in c["text"], c
            assert "Lyrics" in c["label"], c
            say("What does this song mean?"); page.wait_for_timeout(450)
            c = last_send()["context"]; assert c and "made-up line" in c["text"], c
        check("lyrics are sent only for lyric questions, and then with the title, artist and the lyrics", ctx_lyrics_only_when_asked)

        def ctx_setting_off():
            page.evaluate("window.__harness.patchSettings({ai:{contextOnRequest:false}})"); page.wait_for_timeout(300)
            for q in ["What song am I currently listening to?", "Explain these lyrics"]:
                say(q); page.wait_for_timeout(450)
                assert last_send()["context"] is None, (q, last_send()["context"])
            # ...but the Share buttons still work, one message at a time
            page.locator('[data-testid="attach-song"]').click(); say("What is this?"); page.wait_for_timeout(450)
            assert last_send()["context"] and "Lyric Track" in last_send()["context"]["text"]
            say("and again"); page.wait_for_timeout(450)
            assert last_send()["context"] is None, "a Share press applies to one message only"
            page.evaluate("window.__harness.patchSettings({ai:{contextOnRequest:true}})"); page.wait_for_timeout(300)
        check("'Share music only when I ask' off: nothing is sent unless a Share button is pressed", ctx_setting_off)

        def nothing_playing():
            page.evaluate("window.__harness.setSession(null)"); page.wait_for_timeout(500)
            say("What song am I currently listening to?"); page.wait_for_timeout(450)
            c = last_send()["context"]
            assert c and "nothing is playing" in c["text"].lower(), c
        check("with nothing playing the model is told so, rather than left to guess", nothing_playing)
        page.evaluate("window.__harness.setSession({title:'Night Drive', artist:'The Testers', durationMs:200000, positionMs:62000, status:'playing'})"); page.wait_for_timeout(400)

        def ctx_chips():
            page.locator('[data-testid="attach-song"]').click()
            say("hello again"); page.wait_for_timeout(500)
            assert last_send()["context"] and "Night Drive" in last_send()["context"]["text"]
            say("and now plain"); page.wait_for_timeout(500)
            assert last_send()["context"] is None, "the chip is one-shot"
        check("the 'Share song' button attaches context to one message only", ctx_chips)

        def ctx_off():
            page.evaluate("window.__harness.patchSettings({ai:{contextOnRequest:false}})"); page.wait_for_timeout(300)
            say("What song is playing?"); page.wait_for_timeout(500)
            assert last_send()["context"] is None
            expect(page.locator('.attach-note')).to_contain_text("never shared")
            page.evaluate("window.__harness.patchSettings({ai:{contextOnRequest:true}})")
        check("with 'share music only when I ask' off, nothing is ever shared automatically", ctx_off)
        page.evaluate("window.__harness.setSession(null)"); page.wait_for_timeout(300)

        def provider_switch():
            page.evaluate("window.__harness.patchSettings({ai:{provider:'openai'}})"); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="model-chip"]')).to_contain_text("OpenAI")
            expect(page.locator('[data-testid="model-chip"]')).to_contain_text("choose a model")
            say("on openai"); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="chat-error"]')).to_contain_text("openai")
            page.evaluate("window.__harness.patchSettings({ai:{provider:'ollama'}})"); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="model-chip"]')).to_contain_text("llama3.2:latest")
        check("switching provider switches the model too (each provider keeps its own)", provider_switch)

        def history_roundtrip():
            page.locator('[data-testid="chat-error"] >> text=Dismiss').click() if page.locator('[data-testid="chat-error"]').count() else None
            n_msgs = page.locator('[data-testid="msg-user"]').count()
            page.locator('[data-testid="btn-new-chat"]').click(); page.wait_for_timeout(300)
            assert page.locator('[data-testid="msg-user"]').count() == 0
            page.evaluate("window.__harness.setAiScript({chunks:['fresh'], delayMs:30})")
            say("second conversation"); page.wait_for_timeout(700)
            page.locator('[data-testid="btn-history"]').click(); page.wait_for_timeout(300)
            items = page.locator('[data-testid="history-item"]')
            assert items.count() == 2, items.count()
            assert "second conversation" in items.first.inner_text() and "messages" in items.first.inner_text()
            items.nth(1).click(); page.wait_for_timeout(500)
            assert page.locator('[data-testid="msg-user"]').count() == n_msgs, (page.locator('[data-testid="msg-user"]').count(), n_msgs)
            expect(page.locator('[data-testid="msg-user"]').first).to_contain_text("Hi there")
            assert page.locator('time').count() > 0
        check("New chat, history list with timestamps, and reopening an older chat", history_roundtrip)

        def survives_restart():
            page.reload(); page.wait_for_selector('[data-testid="island"]'); page.wait_for_timeout(500)
            goto_assistant()
            assert page.locator('[data-testid="msg-user"]').count() >= 1
            expect(page.locator('[data-testid="msg-user"]').first).to_contain_text("second conversation")
        check("chat history persists across a restart and the latest chat reopens", survives_restart)

        def delete_and_clear():
            page.locator('[data-testid="btn-history"]').click(); page.wait_for_timeout(300)
            page.locator('[data-testid="history-delete"]').first.click(); page.wait_for_timeout(300)
            assert page.locator('[data-testid="history-item"]').count() == 1
            page.locator('[data-testid="clear-history"]').click(); page.locator('[data-testid="confirm-clear-history"]').click(); page.wait_for_timeout(400)
            assert page.locator('[data-testid="history-item"]').count() == 0
            assert page.evaluate("JSON.parse(localStorage.getItem('vex-test-chats')||'[]').length") == 0
        check("a chat can be deleted, and Clear all history (with confirmation) empties the store", delete_and_clear)
        page.locator('[data-testid="btn-history"]').click() if page.locator('[data-testid="history-panel"]').count() else None
        page.wait_for_timeout(200)

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
            assert len(sizes) >= 3 and max(s[0] for s in sizes) >= 540 + 28, sizes
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
            assert abs(w - round(540 * 1.4)) < 3, w
        check("settings: size slider rescales the island (540 -> 756px)", size_scale)

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

        goto_assistant()
        page.locator('[data-testid="btn-new-chat"]').click(); page.wait_for_timeout(300)

        def thinking_is_visible_not_silent():
            # a reasoning model: nothing for a while, then hidden thinking, then the answer
            page.evaluate("window.__harness.setAiScript({chunks:['Done thinking.'], delayMs:50, silentMs:5200, thinkingMs:1800})")
            say("Think about it"); page.wait_for_timeout(600)
            expect(page.locator('[data-testid="typing"]')).to_have_attribute("data-phase", "waiting")
            page.wait_for_timeout(4000)  # ~4.6 s: still silent, but the reader is told it is waiting and for how long
            note = page.locator('[data-testid="wait-note"]').inner_text()
            assert "Waiting for the model" in note and "s" in note, note
            page.wait_for_timeout(1300)  # ~5.9 s: thinking events have started
            expect(page.locator('[data-testid="typing"]')).to_have_attribute("data-phase", "thinking")
            assert "Thinking" in page.locator('[data-testid="wait-note"]').inner_text()
            page.wait_for_timeout(2800)
            assert "Done thinking." in texts("msg-assistant")[-1]
            assert page.locator('[data-testid="typing"]').count() == 0
        check("a slow or reasoning model shows elapsed time and 'Thinking', never a silent screen", thinking_is_visible_not_silent)

        def no_duplicate_requests():
            page.evaluate("window.__harness.setAiScript({chunks:['a','b','c'], delayMs:400})")
            before = len(page.evaluate("window.__harness.aiSends"))
            inp = page.locator('[data-testid="assistant-input"]'); inp.click(); inp.fill("double submit")
            page.keyboard.press("Enter"); page.keyboard.press("Enter"); page.keyboard.press("Enter")
            page.wait_for_timeout(300)
            assert len(page.evaluate("window.__harness.aiSends")) - before == 1, "one message must produce exactly one request"
            page.wait_for_timeout(1600)
        check("pressing Enter repeatedly sends exactly one request", no_duplicate_requests)

        def stop_is_immediate():
            page.evaluate("window.__harness.setAiScript({chunks:Array(40).fill('x '), delayMs:200})")
            say("long answer"); page.wait_for_timeout(700)
            t0 = page.evaluate("performance.now()")
            page.locator('[data-testid="assistant-stop"]').click()
            expect(page.locator('[data-testid="assistant-send"]')).to_be_visible(timeout=1500)
            assert page.evaluate("performance.now()") - t0 < 1000
            assert page.locator('[data-testid="partial-note"]').count() >= 1
        check("Stop ends the answer within a second even mid-stream", stop_is_immediate)

        def fast_stream_is_batched():
            # 400 tiny chunks at 2 ms: React should render far fewer times than there are chunks
            page.evaluate("window.__harness.setAiScript({chunks:Array(400).fill('w '), delayMs:2})")
            page.evaluate("""() => { window.__mut = 0; const el = document.querySelector('[data-testid="chat-scroll"]'); window.__obs = new MutationObserver(m => { window.__mut += m.length; }); window.__obs.observe(el, {childList:true, subtree:true, characterData:true}); }""")
            say("fast"); page.wait_for_timeout(2500)
            muts = page.evaluate("window.__mut"); page.evaluate("window.__obs.disconnect()")
            assert 0 < muts < 400, f"{muts} DOM mutations for 400 chunks"
            assert texts("msg-assistant")[-1].count("w") >= 400
        check("a very fast stream is shown in batches, not one re-render per chunk", fast_stream_is_batched)

        # ---- assistant settings (provider, key, endpoint, connection test, models) ----
        def sget(tid): return sp.locator(f'[data-testid="{tid}"]')
        sp.locator('[data-testid="page-assistant"]').click(); sp.wait_for_timeout(400)

        def ollama_detected():
            expect(sget("test-result")).to_contain_text("2 installed model")
            opts = sget("ai-model").locator("option").all_inner_texts()
            assert "llama3.2:latest" in opts and "qwen2.5:0.5b" in opts, opts
            expect(sget("ollama-endpoint")).to_have_value("http://localhost:11434")
            assert sp.locator('[data-testid="ai-key"]').count() == 0, "Ollama needs no key"
        check("settings: local Ollama is detected and its installed models are listed (no key field)", ollama_detected)

        def pick_installed_model():
            sget("ai-model").select_option("qwen2.5:0.5b"); sp.wait_for_timeout(300)
            assert page.evaluate("JSON.parse(localStorage.getItem('islet-test-settings')).ai.models.ollama") == "qwen2.5:0.5b"
        check("settings: choosing an installed model saves it", pick_installed_model)

        def ollama_down():
            sp.evaluate("window.__harness.setOllama({running:false, models:[]})")
            sget("test-connection").click(); sp.wait_for_timeout(400)
            expect(sget("test-result")).to_contain_text("Can't reach Ollama")
            expect(sget("test-result")).to_contain_text("ollama serve")
        check("settings: Ollama not running gives a helpful error", ollama_down)

        def ollama_empty():
            sp.evaluate("window.__harness.setOllama({running:true, models:[]})")
            sget("test-connection").click(); sp.wait_for_timeout(400)
            expect(sget("test-result")).to_contain_text("ollama pull")
            expect(sget("test-result")).to_contain_text("never downloads")
        check("settings: no installed models explains how to install one and downloads nothing", ollama_empty)
        sp.evaluate("window.__harness.setOllama({running:true, models:['llama3.2:latest','qwen2.5:0.5b']})")

        def key_flow():
            sget("ai-provider").select_option("openai"); sp.wait_for_timeout(400)
            expect(sget("key-state")).to_contain_text("No key saved")
            sget("ai-key").fill("bad-key-999"); sget("save-key").click(); sp.wait_for_timeout(500)
            expect(sget("key-state")).to_contain_text("Key saved")
            expect(sget("test-result")).to_contain_text("rejected the API key")
            assert sget("ai-key").input_value() == "", "the key is never shown again"
            sget("ai-key").fill("good-key-123"); sget("save-key").click(); sp.wait_for_timeout(500)
            expect(sget("test-result")).to_contain_text("Connected to openai")
            assert "openai-model-a" in sget("ai-model").locator("option").all_inner_texts()
            stored = page.evaluate("localStorage.getItem('islet-test-settings')")
            assert "good-key-123" not in stored and "bad-key-999" not in stored, "keys must never be in the settings file"
        check("settings: API key is saved, validated (bad key rejected, good key accepted) and never written to settings", key_flow)

        def key_bad_input():
            sget("ai-key").fill("has a space"); sget("save-key").click(); sp.wait_for_timeout(300)
            expect(sget("key-error")).to_contain_text("can't contain spaces")
        check("settings: a pasted key with spaces/line breaks is refused with an explanation", key_bad_input)

        def key_clear_and_switch():
            sget("clear-key").click(); sp.wait_for_timeout(300)
            expect(sget("key-state")).to_contain_text("No key saved")
            sget("ai-provider").select_option("ollama"); sp.wait_for_timeout(500)
            expect(sget("test-result")).to_contain_text("installed model")
            sget("ai-provider").select_option("openai"); sp.wait_for_timeout(300)
            assert sp.locator('[data-testid="test-result"]').count() == 0, "results of another provider must not linger"
            sget("ai-provider").select_option("ollama"); sp.wait_for_timeout(300)
        check("settings: Clear removes the key; provider switching resets connection state", key_clear_and_switch)

        def privacy_controls():
            sget("ctx-on-request").click(); sp.wait_for_timeout(300)
            assert page.evaluate("JSON.parse(localStorage.getItem('islet-test-settings')).ai.contextOnRequest") is False
            sget("ctx-on-request").click(); sp.wait_for_timeout(300)
            sget("clear-chats").click(); sget("confirm-clear-chats").click(); sp.wait_for_timeout(300)
            expect(sget("clear-msg")).to_contain_text("cleared")
        check("settings: music-sharing toggle persists; Clear chat history asks first and works", privacy_controls)

        def face_preview_drives_island():
            page.locator('[data-testid="assistant-input"]').blur()
            page.mouse.move(5, 380); page.wait_for_timeout(1800)
            if page.locator('[data-testid="btn-new-chat"]').count():  # still expanded: an empty chat shows the face
                page.locator('[data-testid="btn-new-chat"]').click(); page.wait_for_timeout(300)
            sp.locator('[data-testid="page-assistant"]').click(); sp.locator('[data-testid="face-state-thinking"]').click(); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="assistant-face"]').first).to_have_attribute("data-state", "thinking")
        check("settings: face preview plays on the real island", face_preview_drives_island)

        def prompt_default_and_reset():
            sp.locator('[data-testid="page-assistant"]').click(); sp.wait_for_timeout(300)
            default = sget("ai-prompt").input_value()
            assert default.startswith("You are VEX") and "keep replies short" not in default.lower(), default[:80]
            sget("ai-prompt").fill("Answer like a pirate."); sget("ai-prompt").blur(); sp.wait_for_timeout(300)
            assert json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["ai"]["systemPrompt"] == "Answer like a pirate."
            sget("reset-prompt").click(); sp.wait_for_timeout(400)
            assert sget("ai-prompt").input_value() == default
        check("settings: the default prompt asks for substance, a custom one is kept, and Reset restores the default", prompt_default_and_reset)

        # ---- notifications (simulated Windows source; the real filtering rules are Rust-tested) ----
        def collapse():
            page.keyboard.press("Escape"); page.mouse.move(5, 380); page.wait_for_timeout(1500)
        def open_notif_tab():
            isl.hover(); page.wait_for_timeout(900)
            if isl.get_attribute("data-expanded") != "true":
                page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(900)
            page.locator('[data-testid="tab-notifications"]').click(); page.wait_for_timeout(350)
        def notify(**kw):
            page.evaluate("n => window.__harness.notify(n)", kw); page.wait_for_timeout(250)
        def stored():
            return json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["notifications"]
        def to_notif_settings():
            sp.locator('[data-testid="page-notifications"]').click(); sp.wait_for_timeout(300)

        collapse()
        page.evaluate("window.__harness.patchSettings({island:{hoverToExpand:false}})")  # so the pointer resting on the preview does not expand it
        def notif_off_by_default():
            to_notif_settings()
            assert sget("notif-enabled").get_attribute("aria-checked") == "false"
            expect(sget("notif-privacy")).to_contain_text("never sent to an AI provider")
            expect(sget("notif-access")).to_have_count(0)
            notify(appId="mail", appName="Mail", title="While off", body="must be ignored")
            expect(page.locator('[data-testid="view-notification"]')).to_have_count(0)
            expect(isl).to_have_attribute("data-view", "idle")
        check("notifications: off by default, ignore everything while off, privacy statement shown", notif_off_by_default)

        def notif_enable_and_preview():
            sget("notif-enabled").click(); sp.wait_for_timeout(500)
            expect(sget("notif-access")).to_contain_text("given VEX permission")
            notify(appId="mail", appName="Mail", title="Weekly report", body="The numbers are attached.", icon="data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnLz4=")
            page.wait_for_timeout(500)
            expect(isl).to_have_attribute("data-view", "notification")
            v = page.locator('[data-testid="view-notification"]')
            expect(v).to_contain_text("Mail"); expect(v).to_contain_text("Weekly report")
            assert box()["width"] > 300, box()  # wider than the idle pill (size scale may be non-default here)
        check("notifications: enabling shows a compact preview with app and title", notif_enable_and_preview)

        def notif_dismiss_preview():
            page.locator('[data-testid="notif-preview-dismiss"]').click(); page.wait_for_timeout(500)
            expect(page.locator('[data-testid="view-notification"]')).to_have_count(0)
            expect(isl).to_have_attribute("data-expanded", "false")  # dismissing must not expand the island
        check("notifications: dismissing the preview does not expand the island", notif_dismiss_preview)

        def notif_preview_times_out():
            notify(appId="mail", appName="Mail", title="Second", body="")
            expect(isl).to_have_attribute("data-view", "notification")
            page.wait_for_timeout(6600)
            expect(page.locator('[data-testid="view-notification"]')).to_have_count(0)
        check("notifications: an unattended preview goes away by itself but stays in the history", notif_preview_times_out)

        def notif_click_preview_opens_panel():
            notify(appId="chat", appName="Chat", title="Ping 1", body="hello")
            page.locator('[data-testid="view-notification"]').click(); page.wait_for_timeout(1000)
            expect(isl).to_have_attribute("data-expanded", "true")
            expect(page.locator('[data-testid="notifications-panel"]')).to_be_visible()
        check("notifications: clicking the preview opens the full panel", notif_click_preview_opens_panel)

        def notif_panel_groups():
            notify(appId="chat", appName="Chat", title="Ping 2", body="again")
            notify(appId="chat", appName="Chat", title="Ping 3", body="and again")
            groups = page.locator('[data-testid="notif-group"]')
            expect(groups).to_have_count(2)
            assert page.locator('[data-testid="notif-group"][data-app="chat"]').get_attribute("data-count") == "3"
            assert page.locator('[data-testid="notif-group"][data-app="mail"]').get_attribute("data-count") == "2"
            expect(page.locator('[data-testid="notif-group"]').first).to_have_attribute("data-app", "chat")  # newest group first
            expect(page.locator('[data-testid="notif-item"]').first).to_contain_text("Ping 3")
            expect(page.locator('[data-testid="notif-badge"]')).to_have_text("4")  # Weekly report was dismissed; the other four are unread
        check("notifications: panel groups by app, newest first, with an unread badge", notif_panel_groups)

        def notif_group_collapse():
            page.locator('[data-testid="notif-group"][data-app="chat"] [data-testid="notif-group-toggle"]').click(); page.wait_for_timeout(250)
            expect(page.locator('[data-testid="notif-group"][data-app="chat"] [data-testid="notif-item"]')).to_have_count(0)
            page.locator('[data-testid="notif-group"][data-app="chat"] [data-testid="notif-group-toggle"]').click(); page.wait_for_timeout(250)
            expect(page.locator('[data-testid="notif-group"][data-app="chat"] [data-testid="notif-item"]')).to_have_count(3)
        check("notifications: a group can be folded and unfolded", notif_group_collapse)

        def notif_missing_icon_is_a_letter():
            expect(page.locator('[data-testid="notif-group"][data-app="chat"] .n-letter')).to_have_text("C")
        check("notifications: an app without an icon gets a letter tile, not a broken image", notif_missing_icon_is_a_letter)

        def notif_clear_one():
            page.locator('[data-testid="notif-group"][data-app="chat"] [data-testid="notif-clear"]').first.click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="notif-group"][data-app="chat"]')).to_have_attribute("data-count", "2")
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(4)
        check("notifications: one notification can be removed from the history", notif_clear_one)

        def notif_open_app():
            page.locator('[data-testid="notif-group"][data-app="mail"] .notif-main').first.click(); page.wait_for_timeout(300)
            assert len(page.evaluate("window.__harness.notificationOpens")) == 1
            expect(page.locator('[data-testid="notif-group"][data-app="mail"] [data-testid="notif-item"]').first).to_have_attribute("data-unread", "false")
        check("notifications: clicking one opens its app and marks it read", notif_open_app)

        def notif_mark_read_and_clear_all():
            page.locator('[data-testid="notif-mark-read"]').click(); page.wait_for_timeout(250)
            expect(page.locator('[data-testid="notif-badge"]')).to_have_count(0)
            page.locator('[data-testid="notif-clear-all"]').click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(0)
            expect(page.locator('[data-testid="notif-empty"]')).to_be_visible()
        check("notifications: mark all read and clear all history", notif_mark_read_and_clear_all)

        def notif_grouping_off():
            sget("notif-grouping").click(); sp.wait_for_timeout(400)
            for t in ("A", "B"): notify(appId="chat", appName="Chat", title=t, body="")
            page.wait_for_timeout(300)
            expect(page.locator('[data-testid="notif-group"]')).to_have_count(2)
            assert page.locator('[data-testid="notif-group"]').first.get_attribute("data-count") == "1"
            sget("notif-grouping").click(); sp.wait_for_timeout(300)
        check("notifications: with grouping off every notification stands alone", notif_grouping_off)

        def notif_per_app_block():
            expect(sget("notif-app-row")).to_have_count(2)
            sp.locator('[data-testid="notif-app-row"][data-app="chat"] [data-testid="notif-app-toggle"]').click(); sp.wait_for_timeout(500)
            assert stored()["blockedApps"] == ["chat"], stored()
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(0)  # its existing entries are removed
            notify(appId="chat", appName="Chat", title="blocked now", body="")
            notify(appId="mail", appName="Mail", title="still fine", body="")
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(1)
            sp.locator('[data-testid="notif-app-row"][data-app="chat"] [data-testid="notif-app-toggle"]').click(); sp.wait_for_timeout(400)
            assert stored()["blockedApps"] == []
        check("notifications: a single app can be switched off", notif_per_app_block)

        def notif_previews_off():
            sget("notif-previews").click(); sp.wait_for_timeout(500)
            expect(page.locator('[data-testid="notif-item"]').first).to_contain_text("New notification")
            notify(appId="mail", appName="Mail", title="Top secret", body="classified")
            assert "Top secret" not in page.inner_text('[data-testid="notifications-panel"]')
            assert "classified" not in page.inner_text('[data-testid="notifications-panel"]')
            sget("notif-previews").click(); sp.wait_for_timeout(300)
        check("notifications: with previews off the text is hidden, including for what was already stored", notif_previews_off)

        def notif_history_limit():
            sget("notif-limit").select_option("25"); sp.wait_for_timeout(500)
            assert stored()["historyLimit"] == 25
            for i in range(30): page.evaluate("i => window.__harness.notify({appId:'mail',appName:'Mail',title:'n'+i,tsMs:Date.now()+i})", i)
            page.wait_for_timeout(500)
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(25)
            expect(page.locator('[data-testid="notif-item"]').first).to_contain_text("n29")
            sget("notif-limit").select_option("100"); sp.wait_for_timeout(300)
        check("notifications: the history limit is applied", notif_history_limit)

        def notif_clear_from_settings():
            sget("notif-clear-history").click(); sp.wait_for_timeout(500)
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(0)
        check("notifications: history can be cleared from Settings", notif_clear_from_settings)

        def notif_unavailable_is_honest():
            page.evaluate("window.__harness.setNotificationAccess({state:'unavailable', reason:'Windows only lets apps with package identity read notifications.'})"); sp.wait_for_timeout(500)
            expect(sget("notif-access")).to_contain_text("package identity")
            expect(page.locator('[data-testid="notif-status"]')).to_contain_text("package identity")
            page.evaluate("window.__harness.setNotificationAccess({state:'denied'})"); sp.wait_for_timeout(400)
            expect(sget("notif-access")).to_contain_text("Privacy")
            page.evaluate("window.__harness.setNotificationAccess(null)"); sp.wait_for_timeout(300)
        check("notifications: unavailable or denied access is explained, never shown as an empty list", notif_unavailable_is_honest)

        def notif_disable_forgets():
            notify(appId="mail", appName="Mail", title="keep?", body="")
            sget("notif-enabled").click(); sp.wait_for_timeout(500)
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(0)
            expect(page.locator('[data-testid="notif-status"]')).to_contain_text("off")
            expect(sget("notif-apps-empty")).to_be_visible()
            notify(appId="mail", appName="Mail", title="after off", body="")
            expect(page.locator('[data-testid="notif-item"]')).to_have_count(0)
        check("notifications: turning the module off stops it and forgets what it held", notif_disable_forgets)

        def notif_not_in_ai_context():
            n_before = page.evaluate("window.__harness.aiSends.length")
            collapse()
            assert page.evaluate("JSON.stringify(window.__harness.aiSends)").count("secret") == 0
            assert page.evaluate("window.__harness.aiSends.length") == n_before
        check("notifications: nothing from notifications reaches the assistant", notif_not_in_ai_context)
        page.evaluate("window.__harness.patchSettings({island:{hoverToExpand:true}})")
        collapse()

        # ---- developer page + Ollama tuning ----
        def ollama_tuning_defaults():
            sp.locator('[data-testid="page-assistant"]').click(); sp.wait_for_timeout(300)
            assert sget("ollama-think").get_attribute("aria-checked") == "false", "thinking must default to off"
            expect(sget("ollama-keepalive")).to_have_value("30m")
            expect(sget("ollama-ctx")).to_have_value("4096")
        check("settings: Ollama defaults to thinking off, keep-loaded 30 min, 4096-token context", ollama_tuning_defaults)

        def ollama_tuning_saves():
            sget("ollama-think").click(); sget("ollama-keepalive").select_option("2h"); sget("ollama-ctx").select_option("8192"); sp.wait_for_timeout(300)
            s = json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["ai"]
            assert s["ollamaThink"] is True and s["ollamaKeepAlive"] == "2h" and s["ollamaNumCtx"] == 8192, s
            sget("ollama-think").click(); sget("ollama-keepalive").select_option("30m"); sget("ollama-ctx").select_option("4096"); sp.wait_for_timeout(200)
        check("settings: the Ollama tuning choices are saved", ollama_tuning_saves)

        def dev_empty_then_rows():
            sp.locator('[data-testid="page-developer"]').click(); sp.wait_for_timeout(400)
            expect(sget("metrics-empty")).to_be_visible()
            m = {"seq": 7, "startedAtMs": 0, "provider": "ollama", "model": "qwen3.5:4b", "outcome": "done", "errorKind": None, "payloadBytes": 612, "messagesSent": 3,
                 "contextChars": 540, "historyTrimmed": False, "think": False, "numCtx": 4096, "keepAlive": "30m", "prepareMs": 3,
                 "timing": {"headersMs": 900, "firstThinkingMs": None, "firstTokenMs": 1400, "totalMs": 4200, "thinkingChunks": 0, "textChunks": 30, "thinkingChars": 0, "textChars": 120},
                 "usage": {"totalMs": 4100, "loadMs": 0, "promptTokens": 140, "promptEvalMs": 300, "generatedTokens": 30, "generationMs": 2800, "tokensPerSec": 10.7, "doneReason": "stop"}}
            sp.evaluate("m => window.__harness.setMetrics({requests:[m], requestsSent: 1})", m)
            sp.wait_for_timeout(2400)
            row = sget("metric-row")
            expect(row).to_have_count(1)
            expect(sget("m-first")).to_have_text("1.40 s"); expect(sget("m-tps")).to_have_text("10.7")
            expect(sget("requests-sent")).to_contain_text("1 sent")
            body = sp.inner_text('[data-testid="dev-page"]')
            assert "Hello" not in body and "Think about it" not in body, "no conversation text may appear"
        check("developer page shows per-request timings (first word, tokens/s, size) and no message text", dev_empty_then_rows)

        def bench_flow():
            sp.evaluate("window.__harness.setOllama({running:true, models:['qwen3.5:2b','qwen3.5:4b']})")
            rows = []
            for model, ft, tot in (("qwen3.5:2b", 900, 2100), ("qwen3.5:4b", 1800, 5200)):
                rows.append({"model": model, "think": False, "mode": "preload", "ok": True, "error": None, "wallMs": 3000, "headersMs": 3000, "firstThinkingMs": None, "firstTokenMs": None, "visibleChars": 0, "thinkingChars": 0, "payloadBytes": 80, "usage": {"loadMs": 2900}})
                for mode in ("direct", "vex"):
                    rows.append({"model": model, "think": False, "mode": mode, "ok": True, "error": None, "wallMs": tot, "headersMs": 300, "firstThinkingMs": None, "firstTokenMs": ft, "visibleChars": 40, "thinkingChars": 0, "payloadBytes": 300, "usage": {"loadMs": 0, "generatedTokens": 12, "tokensPerSec": 9.5}})
            sp.evaluate("r => window.__harness.setBenchRows(r)", rows)
            sp.locator('[data-testid="page-assistant"]').click(); sp.locator('[data-testid="page-developer"]').click(); sp.wait_for_timeout(500)
            expect(sget("bench-run")).to_be_disabled()
            sget("bench-model").nth(0).check(); sget("bench-model").nth(1).check()
            sget("bench-run").click(); sp.wait_for_timeout(1200)
            calls = sp.evaluate("window.__harness.benchCalls")
            assert calls and sorted(calls[-1]["models"]) == ["qwen3.5:2b", "qwen3.5:4b"] and calls[-1]["includeThinking"] is False, calls
            expect(sget("bench-row")).to_have_count(6)
            modes = sget("bench-row").evaluate_all("els => els.map(e => e.dataset.mode)")
            assert modes.count("direct") == 2 and modes.count("vex") == 2 and modes.count("preload") == 2, modes
            assert "Through VEX" in sp.inner_text('[data-testid="bench-table"]') and "Direct to Ollama" in sp.inner_text('[data-testid="bench-table"]')
        check("developer page: the speed test runs chosen models direct-to-Ollama and through VEX and lists both", bench_flow)
        sp.locator('[data-testid="page-assistant"]').click(); sp.wait_for_timeout(300)

        # ---- clipboard history (simulated source; the real rules are Rust-tested) ----
        collapse()
        def clip_copy(**kw):
            page.evaluate("c => window.__harness.clipboardCopyExternal(c)", kw); page.wait_for_timeout(260)
        def open_clip_tab():
            if isl.get_attribute("data-expanded") != "true":
                isl.hover(); page.wait_for_timeout(700)
            if isl.get_attribute("data-expanded") != "true":
                page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(900)
            page.locator('[data-testid="tab-clipboard"]').click(); page.wait_for_timeout(350)
        def clip_texts():
            return page.locator('[data-testid="clip-item"] .clip-preview').all_inner_texts()
        def cstored():
            return json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["clipboard"]
        def to_clip_settings():
            sp.locator('[data-testid="page-clipboard"]').click(); sp.wait_for_timeout(300)

        def clip_off_by_default():
            to_clip_settings()
            assert sget("clip-enabled").get_attribute("aria-checked") == "false"
            expect(sget("clip-privacy")).to_contain_text("never sent to an AI provider")
            expect(sget("clip-settings-status")).to_contain_text("off")
            clip_copy(kind="text", text="copied while off")
            open_clip_tab()
            expect(page.locator('[data-testid="clip-off"]')).to_be_visible()
            expect(page.locator('[data-testid="clip-rec-dot"]')).to_have_count(0)
        check("clipboard: off by default, nothing recorded while off, privacy statement shown", clip_off_by_default)

        def clip_enable_shows_recording():
            sget("clip-enabled").click(); sp.wait_for_timeout(500)
            expect(sget("clip-settings-status")).to_contain_text("Recording")
            expect(page.locator('[data-testid="clip-rec-dot"]')).to_be_visible()
            st = page.locator('[data-testid="clip-status"]')
            expect(st).to_have_attribute("data-tone", "recording")
            expect(st).to_contain_text("Recording copies")
            clip_texts_before = clip_texts()
            assert clip_texts_before == [], clip_texts_before  # what was copied while off was never kept
        check("clipboard: enabling makes the recording state obvious (tab dot, status line) and keeps nothing from before", clip_enable_shows_recording)

        def clip_records_text_and_image_without_consecutive_duplicates():
            clip_copy(kind="text", text="first note")
            clip_copy(kind="text", text="first note")
            clip_copy(kind="text", text="second\nnote with two lines")
            clip_copy(kind="image", seed=3)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(3)
            assert clip_texts() == ["second\nnote with two lines", "first note"], clip_texts()
            kinds = page.locator('[data-testid="clip-item"]').evaluate_all("els => els.map(e => e.dataset.kind)")
            assert kinds[0] == "image", kinds
            expect(page.locator('[data-testid="clip-thumb"], .clip-thumb')).to_have_count(1)
            assert "1280×720" in page.inner_text('[data-testid="clipboard-panel"]')
        check("clipboard: text and images are listed newest first, an immediate repeat is stored once, images show a thumbnail", clip_records_text_and_image_without_consecutive_duplicates)

        def clip_search_and_copy_back():
            page.locator('[data-testid="clip-search"]').fill("FIRST"); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(1)
            page.locator('[data-testid="clip-copy"]').click(); page.wait_for_timeout(400)
            w = page.evaluate("window.__harness.clipboardWrites")
            assert w[-1]["kind"] == "text" and w[-1]["text"] == "first note", w
            expect(page.locator('[data-testid="clip-item"]').first).to_contain_text("Copied")
            page.locator('[data-testid="clip-search"]').fill("zzz"); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="clip-empty"]')).to_contain_text("No copied text matches")
            page.locator('[data-testid="clip-search"]').fill(""); page.wait_for_timeout(300)
        check("clipboard: search filters text, clicking an entry puts it back on the clipboard", clip_search_and_copy_back)

        def clip_copy_failure_is_reported():
            page.evaluate("window.__harness.failNextClipboardCopy('The clipboard is in use by another program. Try again.')")
            page.locator('[data-testid="clip-copy"]').first.click(); page.wait_for_timeout(400)
            expect(page.locator('[data-testid="clip-note"]')).to_contain_text("in use by another program")
        check("clipboard: a failed copy-back tells the user why", clip_copy_failure_is_reported)

        def clip_pin_delete_clear():
            items = page.locator('[data-testid="clip-item"]')
            last = items.nth(items.count() - 1)
            last.locator('[data-testid="clip-pin"]').click(); page.wait_for_timeout(300)
            assert page.locator('[data-testid="clip-item"]').first.get_attribute("data-pinned") == "true"
            page.locator('[data-testid="clip-clear"]').click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(1)  # only the pinned entry survives Clear
            page.locator('[data-testid="clip-delete"]').click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(0)
            expect(page.locator('[data-testid="clip-clear"]')).to_be_disabled()
        check("clipboard: pin lifts an entry, Clear keeps pinned ones, delete removes one", clip_pin_delete_clear)

        def clip_pause_and_resume():
            to_clip_settings()
            sget("clip-paused").click(); sp.wait_for_timeout(400)
            expect(sget("clip-settings-status")).to_contain_text("Paused")
            expect(page.locator('[data-testid="clip-rec-dot"]')).to_have_count(0)
            expect(page.locator('[data-testid="clip-status"]')).to_have_attribute("data-tone", "paused")
            clip_copy(kind="text", text="while paused")
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(0)
            sget("clip-paused").click(); sp.wait_for_timeout(400)
            clip_copy(kind="text", text="after resume")
            assert clip_texts() == ["after resume"], clip_texts()
        check("clipboard: pausing stops recording (and says so), resuming records again", clip_pause_and_resume)

        def clip_sensitive_and_images_off():
            clip_copy(kind="text", text="gh" + "p_abcdefghijklmnopqrstuvwxyz0123456789")
            expect(page.locator('[data-testid="clip-note"]')).to_contain_text("not saved")
            assert clip_texts() == ["after resume"], clip_texts()
            clip_copy(kind="image", seed=9, sensitive=True)
            assert page.locator('[data-testid="clip-item"][data-kind="image"]').count() == 0
            clip_copy(kind="image", seed=4)
            expect(page.locator('[data-testid="clip-item"][data-kind="image"]')).to_have_count(1)
            sget("clip-images").click(); sp.wait_for_timeout(400)
            expect(page.locator('[data-testid="clip-item"][data-kind="image"]')).to_have_count(0)
            clip_copy(kind="image", seed=5)
            expect(page.locator('[data-testid="clip-item"][data-kind="image"]')).to_have_count(0)
            sget("clip-images").click(); sp.wait_for_timeout(300)
        check("clipboard: private-looking copies are skipped with a notice; turning images off removes and ignores them", clip_sensitive_and_images_off)

        def clip_limit_keeps_pinned():
            sget("clip-max").select_option("10"); sp.wait_for_timeout(300)
            assert cstored()["maxItems"] == 10
            first = page.locator('[data-testid="clip-item"]').last
            first.locator('[data-testid="clip-pin"]').click(); page.wait_for_timeout(300)
            for i in range(14):
                clip_copy(kind="text", text=f"entry {i}")
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(10)
            assert page.locator('[data-testid="clip-item"][data-pinned="true"]').count() == 1
            assert clip_texts()[1] == "entry 13", clip_texts()  # newest unpinned follows the pinned one
        check("clipboard: the history is capped at the chosen size and pinned entries are never trimmed", clip_limit_keeps_pinned)

        def clip_persist_setting_and_clear_all():
            sget("clip-persist").click(); sp.wait_for_timeout(300)
            assert cstored()["persist"] is True
            expect(sget("clip-settings-status")).to_be_visible()
            sget("clip-persist").click(); sp.wait_for_timeout(300)
            assert cstored()["persist"] is False
            sget("clip-clear-all").click(); sp.wait_for_timeout(400)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(0)
            expect(sget("clip-clear-all")).to_be_disabled()
        check("clipboard: persistence is a setting (off by default) and Clear all removes everything including pinned", clip_persist_setting_and_clear_all)

        def clip_disable_forgets_and_stops():
            clip_copy(kind="text", text="will be forgotten")
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(1)
            sget("clip-enabled").click(); sp.wait_for_timeout(500)
            expect(page.locator('[data-testid="clip-off"]')).to_be_visible()
            clip_copy(kind="text", text="after disabling")
            sget("clip-enabled").click(); sp.wait_for_timeout(500)
            expect(page.locator('[data-testid="clip-item"]')).to_have_count(0)  # nothing came back
        check("clipboard: disabling stops recording and forgets the history", clip_disable_forgets_and_stops)

        def clip_never_in_ai_requests():
            sget("clip-enabled").click(); sp.wait_for_timeout(500)
            clip_copy(kind="text", text="SECRET-CLIPBOARD-TEXT")
            sends_before = len(page.evaluate("window.__harness.aiSends"))
            page.locator('[data-testid="tab-assistant"]').click(); page.wait_for_timeout(300)
            say("What did I just copy to the clipboard?"); page.wait_for_timeout(1500)
            sends = page.evaluate("window.__harness.aiSends")
            payload = json.dumps(sends[sends_before:])
            assert len(sends) > sends_before and "SECRET-CLIPBOARD-TEXT" not in payload, payload
            sget("clip-enabled").click(); sp.wait_for_timeout(400)
        check("clipboard: copied text is never part of an assistant request, even when asked about", clip_never_in_ai_requests)

        sp.locator('[data-testid="page-assistant"]').click(); sp.wait_for_timeout(300)

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
            real = [e for e in errors if "favicon" not in e and "Failed to load resource" not in e]
            assert not real, real
        check("no uncaught page errors or console errors", no_errors)

        page.screenshot(path="/tmp/claude-0/-home-claude/bf84f178-7037-5d79-ba4d-45f2658dbe71/scratchpad/final_island.png")
        b.close()
    failed = [r for r in results if not r[1]]
    print(f"\n{len(results)-len(failed)}/{len(results)} UI checks passed")
    sys.exit(1 if failed else 0)

main()
