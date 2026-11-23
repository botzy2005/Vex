"""Notes widget checks (run against `npm run dev`, test backend)."""
import json, sys
from playwright.sync_api import sync_playwright, expect

BASE = "http://localhost:1420"
results = []

def check(name, fn):
    try: fn(); results.append(True); print("PASS", name)
    except Exception as e: results.append(False); print("FAIL", name, "->", str(e)[:600])

def layout(size="l"): return {"version": 1, "widgets": [{"kind": "notes", "size": size, "pinned": False}]}

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": 1100, "height": 760})
    pg = ctx.new_page()
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.goto(BASE + "/hub.html"); pg.wait_for_selector('[data-testid="hub"]')
    T = lambda t: pg.locator(f'[data-testid="{t}"]')

    def fresh(data=None, size="l"):
        pg.evaluate("()=>window.__harness.hub.reset()")
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", layout(size))
        if data is not None:
            pg.evaluate("d=>localStorage.setItem('islet-test-hub-data:notes', JSON.stringify(d))", data)
        pg.reload(); pg.wait_for_selector('[data-testid="widget-notes"][data-view="list"]'); pg.wait_for_timeout(300)
    def stored(): return json.loads(pg.evaluate("localStorage.getItem('islet-test-hub-data:notes')") or '{"notes":[]}')
    def titles(): return T("note-row").evaluate_all("els=>els.map(e=>e.querySelector('.nr-title').innerText.replace('●',''))")
    def write(text):
        T("note-new").click(); pg.wait_for_timeout(150)
        T("note-text").fill(text); pg.wait_for_timeout(100)
        T("note-back").click(); pg.wait_for_timeout(150)
    def doc(*bodies, pinned=()):
        return {"version": 1, "next": len(bodies) + 1, "notes": [
            {"id": f"n{i+1}", "body": t, "pinned": (i + 1) in pinned, "createdAt": 1000 + i, "updatedAt": 1000 + i} for i, t in enumerate(bodies)]}

    def empty_state():
        fresh(); expect(T("notes-empty")).to_contain_text("No notes yet"); assert T("note-row").count() == 0
    check("a fresh Notes card says there are no notes and offers New note", empty_state)

    def create_and_persist():
        fresh(); write("Groceries\nmilk\neggs")
        assert titles() == ["Groceries"]; expect(T("note-row").first.locator(".nr-prev")).to_have_text("milk · eggs")
        pg.wait_for_timeout(700); assert stored()["notes"][0]["body"] == "Groceries\nmilk\neggs"
        pg.reload(); pg.wait_for_selector('[data-testid="widget-notes"][data-view="list"]'); pg.wait_for_timeout(300)
        assert titles() == ["Groceries"]
    check("a note is saved as you type and is there after a reload", create_and_persist)

    def autosave_on_back_immediately():
        fresh(); T("note-new").click(); T("note-text").fill("quick"); T("note-back").click(); pg.reload()
        pg.wait_for_selector('[data-testid="widget-notes"]'); pg.wait_for_timeout(400)
        assert titles() == ["quick"], titles()
    check("leaving the editor saves at once (no 400 ms wait needed)", autosave_on_back_immediately)

    def empty_note_dropped():
        fresh(); T("note-new").click(); pg.wait_for_timeout(200); T("note-back").click(); pg.wait_for_timeout(300)
        assert T("note-row").count() == 0; pg.wait_for_timeout(600); assert stored()["notes"] == []
    check("a note that was never typed into is not kept", empty_note_dropped)

    def edit_existing_moves_to_top():
        fresh(doc("old one", "newer"))
        assert titles() == ["newer", "old one"], titles()
        T("note-row").nth(1).click(); T("note-text").fill("old one, edited"); T("note-back").click(); pg.wait_for_timeout(200)
        assert titles() == ["old one, edited", "newer"], titles()
    check("editing a note moves it to the top of the list", edit_existing_moves_to_top)

    def pin_order():
        fresh(doc("a note", "b note", "c note"))
        T("note-row").last.click(); T("note-pin").click(); expect(T("note-pin")).to_have_text("Unpin"); T("note-back").click(); pg.wait_for_timeout(200)
        assert titles()[0] == "a note", titles()
        assert T("note-row").first.get_attribute("data-pinned") == "true"
        T("note-row").first.click(); T("note-pin").click(); T("note-back").click(); pg.wait_for_timeout(200)
        assert titles() == ["c note", "b note", "a note"], titles()
    check("pinned notes stay on top, and unpinning puts them back by date", pin_order)

    def search_notes():
        fresh(doc("Buy milk", "Call dentist about milk allergy", "Ideas"))
        T("notes-search").fill("milk"); pg.wait_for_timeout(150); assert titles() == ["Call dentist about milk allergy", "Buy milk"], titles()
        T("notes-search").fill("MILK dentist"); pg.wait_for_timeout(150); assert titles() == ["Call dentist about milk allergy"]
        T("notes-search").fill("zzz"); pg.wait_for_timeout(150); expect(T("notes-empty")).to_contain_text("No note matches")
        T("notes-search").fill(""); pg.wait_for_timeout(150); assert T("note-row").count() == 3
    check("search matches every word, ignores case, and says when nothing matches", search_notes)

    def delete_with_confirm():
        fresh(doc("keep me", "delete me"))
        T("note-row").first.click(); expect(T("note-text")).to_have_value("delete me")
        T("note-delete").click(); expect(T("note-delete-yes")).to_be_visible()
        T("note-delete-no").click(); expect(T("note-delete")).to_be_visible(); expect(T("note-text")).to_have_value("delete me")
        T("note-delete").click(); T("note-delete-yes").click(); pg.wait_for_timeout(300)
        assert titles() == ["keep me"], titles()
        pg.wait_for_timeout(600); assert [n["body"] for n in stored()["notes"]] == ["keep me"]
    check("Delete asks first, Keep cancels, and the deletion is saved", delete_with_confirm)

    def limits():
        fresh(); T("note-new").click(); pg.wait_for_timeout(100)
        T("note-text").fill("x" * 9000); pg.wait_for_timeout(200)
        assert len(T("note-text").input_value()) == 8000
        T("note-text").fill("y" * 7800); pg.wait_for_timeout(100)
        expect(T("note-left")).to_have_text("200 characters left"); T("note-back").click()
        full = doc(*[f"note {i}" for i in range(60)])
        fresh(full); T("note-new").click(); pg.wait_for_timeout(200)
        expect(T("notes-message")).to_contain_text("up to 60 notes"); assert T("note-row").count() == 60
    check("text is limited to 8000 characters and the list to 60 notes, with a clear message", limits)

    def corrupt():
        fresh({"version": 1, "next": "x", "notes": [{"id": "n1", "body": "fine"}, {"id": "n1", "body": "dup"}, {"id": "bad", "body": "no"}, 5, None]})
        assert titles() == ["fine"], titles()
        write("another"); assert sorted(titles()) == ["another", "fine"] and stored()["notes"][0]["id"] != "n1"
    check("a damaged saved document is repaired and new ids never collide", corrupt)

    def unicode_roundtrip():
        fresh(); write("日本語のメモ 🎯\nline two <b>not html</b>")
        pg.wait_for_timeout(600); pg.reload(); pg.wait_for_selector('[data-testid="widget-notes"]'); pg.wait_for_timeout(400)
        assert titles() == ["日本語のメモ 🎯"]; expect(T("note-row").first.locator(".nr-prev")).to_have_text("line two <b>not html</b>")
        assert pg.locator("b").count() == 0
    check("non-Latin text and markup-like text are kept as plain text", unicode_roundtrip)

    def sizes():
        for size in ("m", "l"):
            fresh(doc("one", "two", "three"), size=size)
            for t in ("notes-search", "note-new"): expect(T(t)).to_be_visible()
            T("note-row").first.click(); expect(T("note-text")).to_be_visible()
            bb = T("note-text").bounding_box(); cb = pg.locator('[data-testid="widget-card"]').bounding_box()
            assert bb["y"] + bb["height"] <= cb["y"] + cb["height"] + 1 and bb["height"] > 40, (size, bb, cb)
    check("both card sizes show a usable list and editor inside the card", sizes)

    def removed_card_keeps_notes():
        fresh(doc("survivor"))
        pg.evaluate("()=>window.__harness.hub.setLayout({version:1,widgets:[]})"); pg.wait_for_timeout(300)
        assert pg.locator('[data-testid="widget-notes"]').count() == 0
        pg.evaluate("l=>window.__harness.hub.setLayout(l)", layout()); pg.wait_for_timeout(500)
        assert titles() == ["survivor"]
    check("removing and re-adding the card keeps the notes", removed_card_keeps_notes)

    def keyboard():
        fresh(doc("kbd note"))
        T("note-row").first.focus(); pg.keyboard.press("Enter"); pg.wait_for_timeout(150)
        expect(T("note-text")).to_be_focused()
        pg.keyboard.type(" more"); pg.wait_for_timeout(100); assert T("note-text").input_value() == "kbd note more"
    check("a note opens from the keyboard with the cursor in the text", keyboard)

    def failing_save():
        fresh(); T("note-new").click(); pg.evaluate("()=>window.__harness.hub.failNextData('Disk is full')")
        T("note-text").fill("will not save"); pg.wait_for_timeout(900)
        expect(T("notes-error")).to_contain_text("Disk is full")
    check("a failing save is shown, not hidden", failing_save)

    def gallery():
        pg.evaluate("()=>window.__harness.hub.reset()"); pg.evaluate("l=>window.__harness.hub.setLayout(l)", {"version": 1, "widgets": []})
        pg.reload(); pg.wait_for_selector('[data-testid="hub"]'); pg.wait_for_timeout(300)
        T("hub-add").click(); expect(pg.locator('[data-testid="gallery-item"][data-kind="notes"]')).to_be_visible()
    check("the gallery offers Notes", gallery)

    check("no uncaught page errors", lambda: (_ for _ in ()).throw(AssertionError(errors[:3])) if errors else None)
    b.close()

passed = sum(results)
print(f"\n{passed}/{len(results)} notes checks passed")
sys.exit(0 if passed == len(results) else 1)
