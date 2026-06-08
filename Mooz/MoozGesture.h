#ifndef MoozGesture_h
#define MoozGesture_h

#include <stdint.h>

/// Posts a synthetic trackpad magnify (pinch-to-zoom) gesture, serialized as a
/// full IOHID gesture blob and posted to the HID event tap. The complete blob
/// is required for Gecko browsers (Firefox/Zen) to recognize the pinch; loose
/// CGEvent fields only worked in Chrome/Safari.
///
/// phase: IOHID phase bits — 1 = began, 2 = changed, 4 = ended.
void MoozEmitMagnify(double magnification, int32_t phase);

#endif /* MoozGesture_h */
