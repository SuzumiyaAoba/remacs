// A standard macOS text-input control for diagnosing GUI input tools (#292).
// Build: xcrun swiftc cocoa_text_input_control.swift -o /tmp/cocoa-input-control
// Run: /tmp/cocoa-input-control /tmp/new-disposable-input.txt
// Type the same text here and in Remacs, then compare their saved UTF-8 bytes.
// This control saves on every edit. It neither synthesizes input nor tests IME
// delivery to Remacs; use the real IME separately from automation's typeText.
import AppKit

func fail(_ error: String) -> Never {
    FileHandle.standardError.write(Data((error + "\n").utf8))
    exit(1)
}

final class InputControl: NSObject, NSApplicationDelegate, NSTextViewDelegate {
    let output: URL
    var window: NSWindow!
    var editor: NSTextView!

    init(output: URL) {
        self.output = output
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let menu = NSMenu()
        let applicationItem = NSMenuItem()
        let applicationMenu = NSMenu()
        applicationMenu.addItem(NSMenuItem(
            title: "Quit", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
        applicationItem.submenu = applicationMenu
        menu.addItem(applicationItem)
        NSApplication.shared.mainMenu = menu

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 760, height: 460),
            styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "Standard NSTextView input control"
        let scroll = NSScrollView(frame: window.contentView!.bounds)
        scroll.autoresizingMask = [.width, .height]
        editor = NSTextView(frame: scroll.bounds)
        editor.autoresizingMask = [.width, .height]
        editor.isRichText = false
        editor.font = NSFont.systemFont(ofSize: 20)
        editor.delegate = self
        scroll.documentView = editor
        window.contentView = scroll
        window.center()
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(editor)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    func textDidChange(_ notification: Notification) {
        do {
            try editor.string.write(to: output, atomically: true, encoding: .utf8)
        } catch {
            fail("Cannot save input control: \(error)")
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

guard CommandLine.arguments.count == 2 else {
    fail("Usage: cocoa-input-control NEW_DISPOSABLE_OUTPUT_FILE")
}
let output = URL(fileURLWithPath: CommandLine.arguments[1])
do {
    try Data().write(to: output, options: .withoutOverwriting)
} catch {
    fail("Cannot create new input-control file: \(error)")
}
let app = NSApplication.shared
let delegate = InputControl(output: output)
app.setActivationPolicy(.regular)
app.delegate = delegate
app.run()
