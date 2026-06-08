import CoreGraphics
import AppKit
import os

private let logger = Logger(subsystem: "com.dananz.mooz", category: "Interceptor")

// File-based diagnostic logging. OFF by default: this runs inside the
// session event-tap callback, and synchronous per-event disk I/O stalls the
// tap, which backpressures the entire input stream (frozen cursor/clicks).
// Enable only for debugging via `defaults write com.dananz.mooz diag -bool YES`.
private let diagEnabled = UserDefaults.standard.bool(forKey: "diag")
private func diagLog(_ msg: String) {
    guard diagEnabled else { return }
    let path = "/tmp/mooz-debug.log"
    let line = "\(Date()): \(msg)\n"
    if let handle = FileHandle(forWritingAtPath: path) {
        handle.seekToEndOfFile()
        handle.write(line.data(using: .utf8)!)
        handle.closeFile()
    } else {
        FileManager.default.createFile(atPath: path, contents: line.data(using: .utf8))
    }
}

protocol GestureIntercepting: AnyObject {
    func start() throws
    func stop()
    var isRunning: Bool { get }
}

enum GestureInterceptorError: Error {
    case eventTapCreationFailed
    case accessibilityNotGranted
}

final class GestureInterceptor: GestureIntercepting, @unchecked Sendable {
    /// The modifier bits we recognize. The required combo is matched exactly
    /// against these, so e.g. ⇧⌘ requires both Shift and Command and nothing
    /// else, never just Shift.
    static let trackedModifierMask: CGEventFlags = [.maskShift, .maskControl, .maskAlternate, .maskCommand]

    enum InputSource: String, CaseIterable {
        case mouseDrag, scrollWheel
    }

    private let emitter: any MagnificationEmitting
    private let shouldIntercept: () -> Bool
    private let getRequiredModifiers: () -> CGEventFlags
    private let getInputSource: () -> InputSource
    private let getSensitivity: () -> Double
    private let getAnchorCursor: () -> Bool

    /// True while every configured modifier is held. Extra modifiers are
    /// allowed (subset match), so the gesture keeps firing as long as the combo
    /// is down even if another key is brushed — exact-match would drop it.
    private func modifiersMatch(_ flags: CGEventFlags) -> Bool {
        let required = getRequiredModifiers().intersection(Self.trackedModifierMask)
        guard !required.isEmpty else { return false }
        return flags.contains(required)
    }

    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var modifierHeld = false
    private var gestureActive = false
    private var smoothedDeltaY: Double = 0
    private var lastGestureTime: TimeInterval = 0
    private var cursorAnchor: CGPoint = .zero
    private var watchdog: Timer?
    private(set) var isRunning = false

    /// Default local-events suppression interval (0.25s) vs the value that lets
    /// repeated CGWarpMouseCursorPosition calls pin the cursor without the
    /// quarter-second post-warp freeze. Values are Mac Mouse Fix's.
    private static let warpSuppressionInterval: CFTimeInterval = 0.07
    private static let defaultSuppressionInterval: CFTimeInterval = 0.25

    private func setSuppressionInterval(_ interval: CFTimeInterval) {
        guard let src = CGEventSource(stateID: .combinedSessionState) else { return }
        src.localEventsSuppressionInterval = interval
    }

    /// Magnification mapping. Per-pixel rate is multiplied by the user
    /// sensitivity (0.1–3.0); each event's delta is clamped so a single large
    /// movement can't jump the zoom.
    private static let magnificationPerPixel = 0.01
    private static let maxMagnificationPerEvent = 0.15
    private static func clampMagnification(_ value: Double) -> Double {
        max(-maxMagnificationPerEvent, min(maxMagnificationPerEvent, value))
    }

    /// Starts/ends a zoom gesture and freezes the cursor in lockstep. The cursor
    /// is pinned by warping it back to its anchor every frame (see the drag
    /// handler). For that to work smoothly we must shrink the local-events
    /// suppression interval at gesture start (otherwise each warp triggers a
    /// ~0.25s pointer freeze = stutter) and restore it on end. This is exactly
    /// what Mac Mouse Fix ships; they found CGAssociateMouseAndMouseCursorPosition
    /// makes the input deltas "inaccurate and erratic", so we don't use it.
    ///
    /// Warp-based freeze has no global sticky state to strand: the cursor is only
    /// held while moves are actively arriving. The watchdog still force-ends a
    /// stalled gesture so the suppression interval is always restored.
    private func setGestureActive(_ active: Bool) {
        guard active != gestureActive else { return }
        gestureActive = active
        if active {
            smoothedDeltaY = 0
            lastGestureTime = ProcessInfo.processInfo.systemUptime
            if getAnchorCursor() {
                setSuppressionInterval(Self.warpSuppressionInterval)
            }
            diagLog("Gesture active (anchor=\(getAnchorCursor()))")
            startWatchdog()
        } else {
            setSuppressionInterval(Self.defaultSuppressionInterval)
            stopWatchdog()
            smoothedDeltaY = 0
        }
    }

    private func startWatchdog() {
        watchdog?.invalidate()
        watchdog = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] timer in
            guard let self else { timer.invalidate(); return }
            guard self.gestureActive else { return }
            // No movement for a moment → a missed modifier-release or a window
            // stealing focus mid-gesture. End cleanly so the cursor unfreezes.
            if ProcessInfo.processInfo.systemUptime - self.lastGestureTime > 0.2 {
                diagLog("WATCHDOG force-end (no movement 0.2s)")
                self.emitter.emit(magnification: 0, phase: .ended)
                self.setGestureActive(false)
            }
        }
    }

    private func stopWatchdog() {
        watchdog?.invalidate()
        watchdog = nil
    }

    /// Starts the gesture and anchors the cursor at `location` on the first
    /// event while the modifier is held. Shared by drag and scroll paths.
    private func beginGestureIfNeeded(at location: CGPoint) {
        guard !gestureActive else { return }
        cursorAnchor = location
        emitter.emit(magnification: 0, phase: .began)
        setGestureActive(true)
    }

    init(
        emitter: any MagnificationEmitting,
        shouldIntercept: @escaping () -> Bool,
        getRequiredModifiers: @escaping () -> CGEventFlags,
        getInputSource: @escaping () -> InputSource,
        getSensitivity: @escaping () -> Double,
        getAnchorCursor: @escaping () -> Bool
    ) {
        self.emitter = emitter
        self.shouldIntercept = shouldIntercept
        self.getRequiredModifiers = getRequiredModifiers
        self.getInputSource = getInputSource
        self.getSensitivity = getSensitivity
        self.getAnchorCursor = getAnchorCursor
    }

    func start() throws {
        guard AXIsProcessTrusted() else {
            throw GestureInterceptorError.accessibilityNotGranted
        }
        guard eventTap == nil else { return }

        // Clear state a prior run may have stranded if it was killed
        // mid-gesture: re-associate the cursor and restore the suppression
        // interval. Both are harmless if already in their default state.
        CGAssociateMouseAndMouseCursorPosition(1)
        setSuppressionInterval(Self.defaultSuppressionInterval)

        // mouseMoved is tapped so zoom triggers on modifier+move (no button),
        // the Logitech-style gesture. While zooming, the move is consumed so
        // the cursor/page stays anchored. Safe because the per-event disk I/O
        // that previously stalled this callback is now gated off (diagLog).
        let eventMask: CGEventMask = (
            (1 << CGEventType.mouseMoved.rawValue) |
            (1 << CGEventType.leftMouseDragged.rawValue) |
            (1 << CGEventType.rightMouseDragged.rawValue) |
            (1 << CGEventType.scrollWheel.rawValue) |
            (1 << CGEventType.flagsChanged.rawValue)
        )

        let userInfo = Unmanaged.passUnretained(self).toOpaque()

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: GestureInterceptor.tapCallback,
            userInfo: userInfo
        ) else {
            throw GestureInterceptorError.eventTapCreationFailed
        }

        eventTap = tap
        runLoopSource = CFMachPortCreateRunLoopSource(nil, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetCurrent(), runLoopSource, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        isRunning = true
        let tapEnabled = CGEvent.tapIsEnabled(tap: tap)
        diagLog("Event tap started successfully. tapEnabled=\(tapEnabled) runLoop=\(CFRunLoopGetCurrent()!) mainRunLoop=\(CFRunLoopGetMain()!)")

        // Heartbeat: check tap state every 5 seconds
        Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] timer in
            guard let self = self else { timer.invalidate(); return }
            if let tap = self.eventTap {
                let enabled = CGEvent.tapIsEnabled(tap: tap)
                diagLog("Heartbeat: tapEnabled=\(enabled) eventCount=\(self.eventCount)")
                if !enabled {
                    diagLog("⚠️ Tap was disabled! Re-enabling...")
                    CGEvent.tapEnable(tap: tap, enable: true)
                }
            } else {
                diagLog("Heartbeat: eventTap is nil!")
                timer.invalidate()
            }
        }
    }

    func stop() {
        setGestureActive(false) // never leave the cursor locked
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            if let source = runLoopSource {
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
            }
        }
        eventTap = nil
        runLoopSource = nil
        isRunning = false
    }

    private static let tapCallback: CGEventTapCallBack = { proxy, type, event, userInfo in
        guard let userInfo = userInfo else { return Unmanaged.passUnretained(event) }
        let interceptor = Unmanaged<GestureInterceptor>.fromOpaque(userInfo).takeUnretainedValue()
        return interceptor.handleEvent(type: type, event: event, proxy: proxy)
    }

    private var eventCount = 0
    private func handleEvent(type: CGEventType, event: CGEvent, proxy: CGEventTapProxy) -> Unmanaged<CGEvent>? {
        let passthrough = Unmanaged.passUnretained(event)

        // Only log non-mouseMoved events to avoid tap timeout from heavy I/O
        eventCount += 1
        if type != .mouseMoved {
            diagLog("handleEvent #\(eventCount): type=\(type.rawValue)")
        }

        // Re-enable tap only on timeout (callback took too long).
        // Do NOT re-enable on tapDisabledByUserInput — that means the user
        // or system intentionally disabled it (e.g., revoking Accessibility permission).
        if type == .tapDisabledByTimeout {
            diagLog("⚠️ TAP DISABLED BY TIMEOUT — re-enabling")
            if let tap = eventTap {
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            return passthrough
        }

        if type == .tapDisabledByUserInput {
            diagLog("⚠️ TAP DISABLED BY USER INPUT — tearing down")
            // Permission revoked — fully tear down the tap and reset all state
            modifierHeld = false
            setGestureActive(false) // reconnect cursor if it was locked
            if let tap = eventTap {
                CGEvent.tapEnable(tap: tap, enable: false)
                if let source = runLoopSource {
                    CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
                }
            }
            eventTap = nil
            runLoopSource = nil
            isRunning = false
            return passthrough
        }

        // Track modifier key state
        if type == .flagsChanged {
            let flags = event.flags
            let wasHeld = modifierHeld
            modifierHeld = modifiersMatch(flags)
            diagLog("flagsChanged: rawFlags=\(flags.rawValue) required=\(getRequiredModifiers().rawValue) modifierHeld=\(modifierHeld) wasHeld=\(wasHeld)")

            if modifierHeld != wasHeld {
                diagLog("Modifier \(self.modifierHeld ? "PRESSED" : "RELEASED")")
            }

            // End gesture when modifier is released
            if wasHeld && !modifierHeld && gestureActive {
                emitter.emit(magnification: 0, phase: .ended)
                setGestureActive(false)
            }
            return passthrough
        }

        // Check modifier from event flags directly (screen sharing may not send flagsChanged)
        if type == .scrollWheel || type == .mouseMoved || type == .leftMouseDragged || type == .rightMouseDragged {
            let nowHeld = modifiersMatch(event.flags)
            if nowHeld != modifierHeld {
                diagLog("Modifier state updated from event flags: \(nowHeld) (was \(modifierHeld)) rawFlags=\(event.flags.rawValue)")
                modifierHeld = nowHeld
            }
        }

        // If modifier not held or interception disabled, end any active gesture and pass through
        guard modifierHeld, shouldIntercept() else {
            if gestureActive {
                emitter.emit(magnification: 0, phase: .ended)
                setGestureActive(false)
            }
            return passthrough
        }

        let inputSource = getInputSource()
        let sensitivity = getSensitivity()

        switch type {
        case .mouseMoved, .leftMouseDragged, .rightMouseDragged:
            let anchorCursor = getAnchorCursor()
            // In scroll mode a mouse move's only job is to keep the cursor
            // anchored. If anchoring is off there's nothing to do — let it move.
            if inputSource != .mouseDrag && !anchorCursor {
                return passthrough
            }
            // Anchor the cursor whenever the modifier is held, in BOTH modes.
            // (In scroll mode the move only pins the pointer; the wheel zooms.)
            beginGestureIfNeeded(at: event.location)
            if anchorCursor { CGWarpMouseCursorPosition(cursorAnchor) }
            lastGestureTime = ProcessInfo.processInfo.systemUptime

            if inputSource == .mouseDrag {
                // Low-pass the raw pixel delta (the biggest "buttery" factor),
                // map to a magnification delta, clamp per-event to kill spikes.
                let rawDeltaY = event.getDoubleValueField(.mouseEventDeltaY)
                smoothedDeltaY = 0.35 * rawDeltaY + 0.65 * smoothedDeltaY
                let magnification = Self.clampMagnification(-smoothedDeltaY * sensitivity * Self.magnificationPerPixel)
                if abs(magnification) > 0.0002 {
                    emitter.emit(magnification: magnification, phase: .changed)
                }
                diagLog("DRAG zoom: raw=\(rawDeltaY) smooth=\(smoothedDeltaY) mag=\(magnification)")
            }
            return nil // consume so the cursor stays frozen in both modes

        case .scrollWheel:
            guard inputSource == .scrollWheel else { return passthrough }
            // Pixel delta (pointDelta); check BOTH axes since macOS turns
            // modifier+scroll into a horizontal scroll (delta moves to axis 2).
            let deltaY = event.getDoubleValueField(.scrollWheelEventPointDeltaAxis1)
            let deltaX = event.getDoubleValueField(.scrollWheelEventPointDeltaAxis2)
            let deltaPixel = abs(deltaY) > abs(deltaX) ? deltaY : deltaX
            let magnification = Self.clampMagnification(deltaPixel * sensitivity * Self.magnificationPerPixel)

            beginGestureIfNeeded(at: event.location)
            if getAnchorCursor() { CGWarpMouseCursorPosition(cursorAnchor) }
            lastGestureTime = ProcessInfo.processInfo.systemUptime
            if abs(magnification) > 0.0005 {
                emitter.emit(magnification: magnification, phase: .changed)
            }
            diagLog("Scroll zoom: used=\(deltaPixel) mag=\(magnification)")
            return nil

        default:
            return passthrough
        }
    }
}
