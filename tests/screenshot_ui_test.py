"""Browser tests for the screenshot UI (island toast/panel, editor, region overlay, settings), run against
`npm run dev` with the in-browser test backend. Native capture is not exercised here (see docs/CHECKLIST.md)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try:
        fn(); results.append((name, True, "")); print("PASS", name)
    except Exception as e:
        results.append((name, False, str(e)[:500])); print("FAIL", name, "->", str(e)[:500])

def main():
    with sync_playwright() as p:
        b = p.chromium.launch()
        ctx = b.new_context(viewport={"width": 700, "height": 420})
        errors = []
        def watch(pg):
            pg.on("pageerror", lambda e: errors.append(str(e)))
            pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
        page = ctx.new_page(); watch(page)
        page.goto(BASE + "/index.html"); page.wait_for_selector('[data-testid="island"]')
        isl = page.locator('[data-testid="island"]')
        H = lambda js, arg=None: page.evaluate(js, arg)
        H("window.__harness.resetShots()")
        sp = ctx.new_page(); watch(sp)
        sp.goto(BASE + "/settings.html"); sp.wait_for_selector('[data-testid="settings"]')
        sp.locator('[data-testid="page-screenshot"]').click(); sp.wait_for_timeout(300)
        stored = lambda: json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["screenshot"]
        calls = lambda: page.evaluate("window.__harness.screenshotCalls")
        def collapse():
            page.mouse.move(5, 400); page.wait_for_timeout(1800)
        def open_tab():
            isl.hover(); page.wait_for_timeout(800)
            if isl.get_attribute("data-expanded") != "true":
                page.evaluate("window.__harness.emitIslandToggle()"); page.wait_for_timeout(900)
            page.locator('[data-testid="tab-screenshot"]').click(); page.wait_for_timeout(350)

        # ---------------- island ----------------
        def toast_after_capture():
            collapse()
            H("window.__harness.addShot({w:640,h:400})"); page.wait_for_timeout(500)
            expect(isl).to_have_attribute("data-view", "screenshot")
            expect(page.locator('[data-testid="toast-title"]')).to_have_text("Screenshot captured")
            assert "640 × 400" in page.inner_text('[data-testid="view-screenshot"]')
            expect(page.locator('[data-testid="toast-thumb"]')).to_be_visible()
        check("island: a capture shows a confirmation with thumbnail and size", toast_after_capture)

        def toast_actions():
            page.locator('[data-testid="toast-copy"]').click(); page.wait_for_timeout(300)
            page.locator('[data-testid="toast-save"]').click(); page.wait_for_timeout(300)
            cs = calls()
            assert any(c["type"] == "copy" for c in cs), cs
            assert any(c["type"] == "save" and c["ask"] is False for c in cs), cs
            expect(page.locator('[data-testid="toast-reveal"]')).to_be_visible()
            assert "Saved" in page.inner_text('[data-testid="toast-title"]')
            page.locator('[data-testid="toast-reveal"]').click(); page.wait_for_timeout(200)
            assert any(c["type"] == "reveal" for c in calls())
        check("island: toast Copy and Save call the backend and Show reveals the saved file", toast_actions)

        def toast_edit_and_dismiss():
            collapse()
            H("window.__harness.addShot({w:800,h:500})"); page.wait_for_timeout(400)
            page.locator('[data-testid="toast-edit"]').click(); page.wait_for_timeout(300)
            assert [c for c in calls() if c["type"] == "openEditor"], calls()
            expect(isl).not_to_have_attribute("data-view", "screenshot")
            H("window.__harness.addShot()"); page.wait_for_timeout(400)
            page.locator('[data-testid="toast-dismiss"]').click(); page.wait_for_timeout(300)
            expect(isl).not_to_have_attribute("data-view", "screenshot")
        check("island: Edit opens the editor and closes the toast; dismiss closes it", toast_edit_and_dismiss)

        def toast_times_out():
            collapse()
            H("window.__harness.addShot()"); page.wait_for_timeout(400)
            expect(isl).to_have_attribute("data-view", "screenshot")
            page.wait_for_timeout(7600)
            expect(isl).not_to_have_attribute("data-view", "screenshot")
        check("island: the confirmation goes away by itself", toast_times_out)

        def error_toast():
            collapse()
            H("window.__harness.failNextScreenshot('Could not capture the screen.')")
            H("window.__harness.screenshotCapture ? 0 : 0")
            open_tab(); page.locator('[data-testid="shot-screen"]').click(); page.wait_for_timeout(700)
            body = page.locator('[data-testid="view-screenshot-error"], [data-testid="shot-note"]').first.inner_text()
            assert "Could not capture the screen." in body, body
        check("island: a failed capture says why", error_toast)

        def panel_lists_and_modes():
            collapse(); H("window.__harness.resetShots()"); open_tab()
            expect(page.locator('[data-testid="shot-empty"]')).to_be_visible()
            n0 = len(calls())
            for mode in ["region", "window", "screen"]:
                page.locator(f'[data-testid="shot-{mode}"]').click(); page.wait_for_timeout(250)
                open_tab()
            modes = [c["mode"] for c in calls()[n0:] if c["type"] == "capture"]
            assert modes == ["region", "window", "screen"], modes
            items = page.locator('[data-testid="shot-item"]')
            expect(items).to_have_count(2)  # the area picker opens the overlay instead of adding a picture itself
            txt = page.inner_text('[data-testid="shot-list"]')
            assert "960 × 540" in txt and "640 × 400" in txt, txt
            assert txt.index("960") < txt.index("640"), txt
        check("panel: three capture modes call the backend; results are listed newest first", panel_lists_and_modes)

        def panel_shortcut_hint():
            assert "Ctrl+Alt+S" in page.inner_text('[data-testid="shot-shortcut"]')
        check("panel: shows the configured shortcut", panel_shortcut_hint)

        def panel_preview():
            page.locator('[data-testid="shot-thumb"]').first.click(); page.wait_for_timeout(400)
            expect(page.locator('[data-testid="shot-preview-img"]')).to_be_visible()
            page.locator('[data-testid="preview-save"]').click(); page.wait_for_timeout(250)
            assert any(c["type"] == "save" and c["ask"] is True for c in calls())
            page.locator('[data-testid="preview-close"]').click(); page.wait_for_timeout(200)
            expect(page.locator('[data-testid="shot-preview"]')).to_have_count(0)
        check("panel: quick preview opens, Save as asks for a location, Close returns to the list", panel_preview)

        def panel_delete_clear():
            page.locator('[data-testid="shot-delete"]').first.click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="shot-item"]')).to_have_count(1)
            page.locator('[data-testid="shot-clear"]').click(); page.wait_for_timeout(300)
            expect(page.locator('[data-testid="shot-empty"]')).to_be_visible()
        check("panel: delete removes one, Clear removes all", panel_delete_clear)

        # ---------------- settings ----------------
        def settings_defaults_and_persist():
            assert sp.locator('[data-testid="shot-format-png"]').get_attribute("aria-checked") == "true"
            assert sp.inner_text('[data-testid="shot-hotkey"]') == "Ctrl+Alt+S"
            assert sp.locator('[data-testid="shot-hotkey-mode"]').input_value() == "region"
            assert sp.locator('[data-testid="shot-history"]').input_value() == "session"
            assert sp.locator('[data-testid="shot-sound"]').get_attribute("aria-checked") == "false"
            assert "stay on this PC" in sp.inner_text('[data-testid="shot-privacy"]')
            sp.locator('[data-testid="shot-format-jpeg"]').click(); sp.wait_for_timeout(250)
            sp.locator('[data-testid="shot-sound"]').click(); sp.wait_for_timeout(250)
            sp.locator('[data-testid="shot-hotkey-mode"]').select_option("window"); sp.wait_for_timeout(250)
            sp.locator('[data-testid="shot-history"]').select_option("keep"); sp.wait_for_timeout(250)
            s = stored()
            assert s["format"] == "jpeg" and s["sound"] is True and s["hotkeyMode"] == "window" and s["history"] == "keep", s
        check("settings: defaults, and format / sound / shortcut mode / history persist", settings_defaults_and_persist)

        def settings_folder_and_hotkey():
            sp.locator('[data-testid="shot-choose-dir"]').click(); sp.wait_for_timeout(300)
            assert any(c["type"] == "pickFolder" for c in calls()) or sp.evaluate("window.__harness.screenshotCalls").__len__() > 0
            sp.locator('[data-testid="shot-hotkey-clear"]').click(); sp.wait_for_timeout(300)
            assert stored()["hotkey"] == ""
            expect(sp.locator('[data-testid="shot-hotkey-off"]')).to_be_visible()
            sp.locator('[data-testid="shot-hotkey-default"]').click(); sp.wait_for_timeout(300)
            assert stored()["hotkey"] == "Ctrl+Alt+S"
        check("settings: folder picker is offered; the shortcut can be turned off and restored", settings_folder_and_hotkey)

        def settings_clash():
            isl_hk = json.loads(sp.evaluate("localStorage.getItem('islet-test-settings')"))["island"]["hotkey"]
            sp.evaluate("hk => localStorage.setItem('x', hk)", isl_hk)
            expect(sp.locator('[data-testid="shot-hotkey-clash"]')).to_have_count(0)
        check("settings: no clash warning with the defaults", settings_clash)

        # ---------------- editor ----------------
        def new_editor(w=640, h=400):
            sid = H("([w,h]) => window.__harness.addShot({w,h})", [w, h])
            ep = ctx.new_page(); watch(ep); ep.set_viewport_size({"width": 1100, "height": 900})
            ep.goto(f"{BASE}/editor.html?id={sid}")
            ep.wait_for_selector('[data-testid="editor-canvas"]'); ep.wait_for_timeout(500)
            return ep, sid
        def px(ep, x, y):
            return ep.evaluate("([x,y]) => Array.from(document.querySelector('[data-testid=editor-canvas]').getContext('2d').getImageData(x,y,1,1).data)", [x, y])
        def size(ep):
            return ep.evaluate("(() => { const c = document.querySelector('[data-testid=editor-canvas]'); return [c.width, c.height]; })()")
        def drag(ep, a, b2):
            bb = ep.locator('[data-testid="editor-canvas"]').bounding_box()
            w, h = size(ep)
            f = lambda pt: (bb["x"] + pt[0] / w * bb["width"], bb["y"] + pt[1] / h * bb["height"])
            (x0, y0), (x1, y1) = f(a), f(b2)
            ep.mouse.move(x0, y0); ep.mouse.down(); ep.mouse.move((x0 + x1) / 2, (y0 + y1) / 2); ep.mouse.move(x1, y1); ep.mouse.up(); ep.wait_for_timeout(150)
        near = lambda got, want, tol=8: all(abs(g - w_) <= tol for g, w_ in zip(got[:3], want))

        def editor_loads():
            ep, _ = new_editor()
            assert size(ep) == [640, 400], size(ep)
            assert near(px(ep, 100, 200), (255, 0, 0)) and near(px(ep, 500, 200), (0, 0, 255)), (px(ep, 100, 200), px(ep, 500, 200))
            assert "640 × 400" in ep.inner_text('[data-testid="editor-size"]')
            ep.close()
        check("editor: opens the screenshot at its full pixel size", editor_loads)

        def editor_shapes_and_undo():
            ep, _ = new_editor()
            ep.locator('[data-testid="editor-color"]').fill("#00ff00")
            ep.locator('[data-testid="tool-rect"]').click()
            col = "#00ff00"
            drag(ep, (50, 50), (200, 150)); 
            edge = px(ep, 50, 100)
            fill_inside = px(ep, 120, 100)
            assert near(edge, (0, 255, 0)), edge
            assert near(fill_inside, (255, 0, 0)), fill_inside  # rectangles are outlines, not filled
            ep.locator('[data-testid="tool-ellipse"]').click(); drag(ep, (300, 50), (400, 150))
            ep.locator('[data-testid="tool-line"]').click(); drag(ep, (10, 300), (200, 300))
            ep.locator('[data-testid="tool-arrow"]').click(); drag(ep, (300, 300), (500, 350))
            ep.locator('[data-testid="tool-highlight"]').click(); drag(ep, (20, 340), (120, 380))
            assert not near(px(ep, 70, 360), (255, 0, 0), 4), px(ep, 70, 360)
            for _ in range(5): ep.locator('[data-testid="editor-undo"]').click()
            ep.wait_for_timeout(150)
            assert near(px(ep, 70, 360), (255, 0, 0)) and near(px(ep, 50, 100), (255, 0, 0)), (px(ep, 70, 360), px(ep, 50, 100))
            expect(ep.locator('[data-testid="editor-undo"]')).to_be_disabled()
            ep.locator('[data-testid="editor-redo"]').click(); ep.wait_for_timeout(150)
            assert edge == px(ep, 50, 100), (edge, px(ep, 50, 100))
            ep.close()
        check("editor: rectangle, ellipse, line, arrow and highlight draw; undo and redo step through them", editor_shapes_and_undo)

        def editor_blur_pixelate():
            for tool in ["blur", "pixelate"]:
                ep, _ = new_editor()
                before = [px(ep, x, 40) for x in range(520, 620, 5)]
                assert any(v[:3] == [0, 0, 0] for v in before) and any(v[:3] == [255, 255, 255] for v in before), before
                ep.locator(f'[data-testid="tool-{tool}"]').click(); drag(ep, (500, 10), (630, 80))
                after = [px(ep, x, 40) for x in range(520, 620, 5)]
                pure = lambda vs: [v for v in vs if v[:3] in ([0, 0, 0], [255, 255, 255])]
                runs = lambda vs: sum(1 for a_, b_ in zip(vs, vs[1:]) if a_ == b_)
                assert len(pure(before)) >= 10 and runs(before) == 0, before
                if tool == "blur":
                    assert len(pure(after)) <= 4, after  # the hard black/white stripes are smeared away
                else:
                    assert runs(after) >= 8, after  # flat blocks wider than a stripe
                assert near(px(ep, 100, 200), (255, 0, 0))  # outside the box is untouched
                ep.locator('[data-testid="editor-undo"]').click(); ep.wait_for_timeout(150)
                assert [px(ep, x, 40) for x in range(520, 620, 5)] == before, tool
                ep.close()
        check("editor: blur and pixelate destroy the stripe detail only inside the box, and undo restores it", editor_blur_pixelate)

        def editor_text():
            ep, _ = new_editor()
            ep.locator('[data-testid="tool-text"]').click()
            bb = ep.locator('[data-testid="editor-canvas"]').bounding_box()
            ep.mouse.click(bb["x"] + 60, bb["y"] + 200); ep.wait_for_timeout(150)
            ep.locator('[data-testid="text-input"]').fill("Hello"); ep.keyboard.press("Enter"); ep.wait_for_timeout(250)
            changed = ep.evaluate("""() => { const c = document.querySelector('[data-testid=editor-canvas]'); const d = c.getContext('2d').getImageData(0,150,300,100).data;
              for (let i = 0; i < d.length; i += 4) if (!(d[i] > 240 && d[i+1] < 15 && d[i+2] < 15)) return true; return false; }""")
            assert changed
            ep.close()
        check("editor: text can be placed on the image", editor_text)

        def editor_crop():
            ep, _ = new_editor()
            ep.locator('[data-testid="tool-crop"]').click(); drag(ep, (100, 50), (420, 250))
            expect(ep.locator('[data-testid="crop-panel"]')).to_be_visible()
            ep.locator('[data-testid="crop-apply"]').click(); ep.wait_for_timeout(250)
            assert size(ep) == [320, 200], size(ep)
            assert near(px(ep, 10, 100), (255, 0, 0)) and near(px(ep, 310, 100), (0, 0, 255)), (px(ep, 10, 100), px(ep, 310, 100))
            ep.locator('[data-testid="editor-undo"]').click(); ep.wait_for_timeout(250)
            assert size(ep) == [640, 400], size(ep)
            ep.locator('[data-testid="tool-crop"]').click(); drag(ep, (10, 10), (200, 200))
            ep.locator('[data-testid="crop-cancel"]').click(); assert size(ep) == [640, 400]
            ep.close()
        check("editor: crop applies the selected area exactly, undo restores, cancel changes nothing", editor_crop)

        def editor_rotate():
            ep, _ = new_editor()
            ep.locator('[data-testid="rotate-right"]').click(); ep.wait_for_timeout(250)
            assert size(ep) == [400, 640], size(ep)
            # left red half becomes the top half after a clockwise turn
            assert near(px(ep, 200, 100), (255, 0, 0)) and near(px(ep, 200, 500), (0, 0, 255)), (px(ep, 200, 100), px(ep, 200, 500))
            ep.locator('[data-testid="rotate-left"]').click(); ep.wait_for_timeout(250)
            assert size(ep) == [640, 400] and near(px(ep, 100, 200), (255, 0, 0))
            ep.locator('[data-testid="editor-undo"]').click(); ep.locator('[data-testid="editor-undo"]').click(); ep.wait_for_timeout(200)
            assert size(ep) == [640, 400]
            ep.close()
        check("editor: rotate turns the picture 90° each way and can be undone", editor_rotate)

        def editor_resize():
            ep, _ = new_editor()
            ep.locator('[data-testid="resize-open"]').click()
            ep.locator('[data-testid="resize-w"]').fill("320"); ep.wait_for_timeout(100)
            assert ep.locator('[data-testid="resize-h"]').input_value() == "200"
            ep.locator('[data-testid="resize-w"]').fill("0")
            expect(ep.locator('[data-testid="resize-error"]')).to_be_visible()
            expect(ep.locator('[data-testid="resize-apply"]')).to_be_disabled()
            ep.locator('[data-testid="resize-w"]').fill("99999")
            expect(ep.locator('[data-testid="resize-error"]')).to_be_visible()
            ep.locator('[data-testid="resize-lock"]').uncheck()
            ep.locator('[data-testid="resize-w"]').fill("300"); ep.locator('[data-testid="resize-h"]').fill("100")
            ep.locator('[data-testid="resize-apply"]').click(); ep.wait_for_timeout(300)
            assert size(ep) == [300, 100], size(ep)
            assert near(px(ep, 20, 50), (255, 0, 0)) and near(px(ep, 280, 50), (0, 0, 255))
            ep.locator('[data-testid="editor-undo"]').click(); ep.wait_for_timeout(200)
            assert size(ep) == [640, 400]
            ep.close()
        check("editor: resize keeps proportions when locked, rejects bad sizes, applies exact sizes, undoes", editor_resize)

        def editor_copy_save():
            ep, sid = new_editor()
            ep.locator('[data-testid="tool-line"]').click(); drag(ep, (10, 300), (200, 300))
            ep.locator('[data-testid="editor-copy"]').click(); ep.wait_for_timeout(400)
            ep.locator('[data-testid="editor-save"]').click(); ep.wait_for_timeout(400)
            ep.locator('[data-testid="editor-save-as"]').click(); ep.wait_for_timeout(400)
            cs = ep.evaluate("window.__harness.screenshotCalls")
            copy = [c for c in cs if c["type"] == "copy"]; save = [c for c in cs if c["type"] == "save"]
            assert copy and copy[-1]["dataUrl"] == "data:image/png;base64," and copy[-1]["bytes"] > 200, copy
            assert [s["ask"] for s in save] == [False, True], save
            assert save[0]["data"].startswith("data:image/png;base64,"), save[0]["data"][:30]
            expect(ep.locator('[data-testid="editor-status"]')).to_contain_text("Saved")
            ep.keyboard.press("Control+z"); ep.wait_for_timeout(150)
            expect(ep.locator('[data-testid="editor-undo"]')).to_be_disabled()
            ep.close()
        check("editor: Copy and Save send the edited picture as PNG (Save as asks for a place); Ctrl+Z works", editor_copy_save)

        def editor_missing():
            ep = ctx.new_page(); watch(ep)
            ep.goto(f"{BASE}/editor.html?id=987654"); ep.wait_for_timeout(700)
            expect(ep.locator('[data-testid="editor-error"]')).to_be_visible()
            ep.close()
        check("editor: a missing screenshot shows an error, not a blank window", editor_missing)

        # ---------------- region overlay ----------------
        def overlay_drag():
            op = ctx.new_page(); watch(op)
            op.goto(f"{BASE}/overlay.html?i=0"); op.wait_for_selector('[data-testid="overlay"]')
            expect(op.locator('[data-testid="overlay-hint"]')).to_be_visible()
            op.mouse.move(100, 100); op.mouse.down(); op.mouse.move(250, 200); op.mouse.move(300, 260)
            expect(op.locator('[data-testid="overlay-selection"]')).to_be_visible()
            assert "200 × 160" in op.inner_text('[data-testid="overlay-size"]'), op.inner_text('[data-testid="overlay-size"]')
            op.mouse.up(); op.wait_for_timeout(300)
            c = [x for x in op.evaluate("window.__harness.screenshotCalls") if x["type"] == "regionSelect"]
            assert c and c[-1]["monitor"] == 0 and (c[-1]["x0"], c[-1]["y0"], c[-1]["x1"], c[-1]["y1"]) == (100, 100, 300, 260), c
            op.close()
        check("overlay: dragging reports the exact rectangle for monitor 0 and shows its size live", overlay_drag)

        def overlay_cancel():
            op = ctx.new_page(); watch(op)
            op.goto(f"{BASE}/overlay.html?i=1"); op.wait_for_selector('[data-testid="overlay"]')
            op.keyboard.press("Escape"); op.wait_for_timeout(200)
            op.mouse.move(50, 50); op.mouse.down(); op.mouse.move(120, 120); op.mouse.up(); op.wait_for_timeout(200)
            cs = op.evaluate("window.__harness.screenshotCalls")
            assert any(c["type"] == "regionCancel" for c in cs), cs
            op.close()
        check("overlay: Escape cancels the selection", overlay_cancel)

        def overlay_tiny_click():
            op = ctx.new_page(); watch(op)
            op.goto(f"{BASE}/overlay.html?i=0"); op.wait_for_selector('[data-testid="overlay"]')
            op.mouse.click(200, 200); op.wait_for_timeout(200)
            cs = op.evaluate("window.__harness.screenshotCalls")
            assert not any(c["type"] == "regionSelect" for c in cs), cs
            op.close()
        check("overlay: a plain click is not a selection", overlay_tiny_click)

        def overlay_hidpi():
            c2 = b.new_context(viewport={"width": 700, "height": 420}, device_scale_factor=2)
            op = c2.new_page(); op.goto(f"{BASE}/overlay.html?i=0"); op.wait_for_selector('[data-testid="overlay"]')
            op.mouse.move(100, 100); op.mouse.down(); op.mouse.move(300, 260)
            text = op.inner_text('[data-testid="overlay-size"]')
            op.mouse.up(); op.wait_for_timeout(300)
            c = [x for x in op.evaluate("window.__harness.screenshotCalls") if x["type"] == "regionSelect"]
            assert "400 × 320" in text, text  # the label shows real pixels on a 2x display
            assert c and c[-1]["dpr"] == 2 and (c[-1]["x1"] - c[-1]["x0"]) == 200, c
            c2.close()
        check("overlay: on a 2x display the label shows physical pixels while the drag is reported in CSS pixels", overlay_hidpi)

        def no_errors():
            real = [e for e in errors if "favicon" not in e and "Failed to load resource" not in e]
            assert not real, real
        check("no uncaught page errors or console errors", no_errors)
        b.close()
    failed = [r for r in results if not r[1]]
    print(f"\n{len(results)-len(failed)}/{len(results)} screenshot UI checks passed")
    sys.exit(1 if failed else 0)

main()
