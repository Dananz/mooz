import AppKit
import SwiftUI
import os

private let logger = Logger(subsystem: "com.dananz.mooz", category: "ZoomManager")

private let diagEnabled = UserDefaults.standard.bool(forKey: "diag")
private func diagLog(_ msg: String) {
    guard diagEnabled else { return }
    let path = "/tmp/mooz-debug.log"
    let line = "\(Date()): [ZoomManager] \(msg)\n"
    if let handle = FileHandle(forWritingAtPath: path) {
        handle.seekToEndOfFile()
        handle.write(line.data(using: .utf8)!)
        handle.closeFile()
    } else {
        FileManager.default.createFile(atPath: path, contents: line.data(using: .utf8))
    }
}

@Observable
@MainActor
final class ZoomManager {
    static let shared = ZoomManager()

    private(set) var isRunning = false
    private(set) var accessibilityGranted = false

    private var interceptor: (any GestureIntercepting)?
    private let emitter: any MagnificationEmitting
    private let blocklistManager: BlocklistManager
    private var dispatchTimer: DispatchSourceTimer?

    init(
        emitter: any MagnificationEmitting = MagnificationEmitter(),
        blocklistManager: BlocklistManager = BlocklistManager()
    ) {
        self.emitter = emitter
        self.blocklistManager = blocklistManager
        checkAccessibility()
        startAccessibilityPolling()
    }

    func checkAccessibility() {
        let trusted = AXIsProcessTrusted()
        diagLog("checkAccessibility: AXIsProcessTrusted()=\(trusted)")
        accessibilityGranted = trusted
        if !trusted {
            startAccessibilityPolling()
        }
    }

    /// Prompts macOS to show Mooz in the Accessibility list (adds the entry if missing).
    /// After each rebuild, the binary changes and macOS invalidates the old permission —
    /// this ensures the new binary gets re-registered in System Settings.
    func requestAccessibility() {
        // kAXTrustedCheckOptionPrompt is not concurrency-safe in Swift 6,
        // so we use the raw string key directly
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        accessibilityGranted = AXIsProcessTrustedWithOptions(options)
    }

    /// Polls AXIsProcessTrusted() every 2 seconds until permission is granted,
    /// then auto-starts the interceptor and stops polling.
    /// Can be called multiple times safely — restarts polling if not already running.
    func startAccessibilityPolling() {
        guard !accessibilityGranted else { return }
        guard dispatchTimer == nil else { return } // already polling
        diagLog("Starting accessibility polling")
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + 1, repeating: 1.5)
        timer.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                let trusted = AXIsProcessTrusted()
                diagLog("Poll: AXIsProcessTrusted()=\(trusted)")
                if trusted != self.accessibilityGranted {
                    self.accessibilityGranted = trusted
                }
                if trusted {
                    self.dispatchTimer?.cancel()
                    self.dispatchTimer = nil
                    diagLog("Permission detected! Starting interceptor...")
                    self.startIfEnabled()
                }
            }
        }
        timer.resume()
        dispatchTimer = timer
    }

    func openAccessibilitySettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    func startIfEnabled() {
        // UserDefaults returns false when key doesn't exist, so treat absence as enabled (default on)
        let defaults = UserDefaults.standard
        let isEnabled: Bool
        if defaults.object(forKey: "isEnabled") == nil {
            isEnabled = true
        } else {
            isEnabled = defaults.bool(forKey: "isEnabled")
        }

        guard isEnabled else {
            stop()
            return
        }
        start()
    }

    func start() {
        diagLog("start() called. interceptor=\(interceptor != nil ? "exists" : "nil") accessibilityGranted=\(accessibilityGranted)")
        guard interceptor == nil else { return }
        checkAccessibility()
        guard accessibilityGranted else {
            diagLog("start() aborted — no accessibility permission")
            return
        }

        // Build a non-MainActor shouldIntercept closure that reads UserDefaults
        // (thread-safe) without going through NSWorkspace (which requires MainActor).
        // We snapshot the current frontmost bundle ID from the main actor before
        // the event tap fires; for ongoing checks we rely on a nonisolated helper.
        let shouldIntercept: @Sendable () -> Bool = {
            // Read app list and mode from UserDefaults — thread-safe reads
            let raw = UserDefaults.standard.string(forKey: "listMode") ?? "blocklist"
            let mode = BlocklistManager.ListMode(rawValue: raw) ?? .blocklist

            guard let data = UserDefaults.standard.data(forKey: "appList"),
                  let appList = try? JSONDecoder().decode([AppEntry].self, from: data) else {
                // No list configured — always intercept
                return true
            }

            // NSWorkspace.shared.frontmostApplication is documented as main-thread
            // only. We use MainActor.assumeIsolated here because CGEventTap callbacks
            // run on the main run loop (same thread as the main actor in a standard
            // AppKit/SwiftUI app). This is safe as long as the tap runs on the main
            // run loop — which is where we add it in start().
            let bundleId = MainActor.assumeIsolated {
                NSWorkspace.shared.frontmostApplication?.bundleIdentifier
            }

            guard let bundleId else { return true }
            let isInList = appList.contains { $0.bundleId == bundleId }

            switch mode {
            case .blocklist:
                return !isInList
            case .allowlist:
                return isInList
            }
        }

        let interceptor = GestureInterceptor(
            emitter: emitter,
            shouldIntercept: shouldIntercept,
            getRequiredModifiers: {
                ModifierStore.currentFlags()
            },
            getInputSource: {
                let raw = UserDefaults.standard.string(forKey: "inputSource") ?? "mouseDrag"
                return GestureInterceptor.InputSource(rawValue: raw) ?? .mouseDrag
            },
            getSensitivity: {
                let val = UserDefaults.standard.double(forKey: "sensitivity")
                return val > 0 ? val : 0.5
            },
            getAnchorCursor: {
                // Absent key defaults to ON.
                let d = UserDefaults.standard
                return d.object(forKey: "anchorCursor") == nil ? true : d.bool(forKey: "anchorCursor")
            }
        )

        do {
            try interceptor.start()
            self.interceptor = interceptor
            isRunning = true
            diagLog("Interceptor started successfully! isRunning=true")
        } catch {
            diagLog("Failed to start interceptor: \(error)")
            isRunning = false
        }
    }

    func stop() {
        interceptor?.stop()
        interceptor = nil
        isRunning = false
    }

    func toggle() {
        if isRunning {
            stop()
            UserDefaults.standard.set(false, forKey: "isEnabled")
        } else {
            UserDefaults.standard.set(true, forKey: "isEnabled")
            start()
        }
    }
}
