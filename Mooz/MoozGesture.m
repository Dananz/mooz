#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <mach/mach_time.h>
#import <string.h>

#import "MoozGesture.h"
#import "MoozHIDLayout.h"

/*
 * MoozGesture.m
 *
 * Original (clean-room) synthesis of a macOS trackpad magnify gesture.
 *
 * AppKit/Gecko only treat a CGEvent as a real pinch when it carries the full
 * IOHID gesture payload a physical trackpad would send: a queue element that
 * collects a digitizer "hand" parent, a vendor token, and a set of gesture
 * parameter fields. We build that payload by hand and hand it to
 * CGEventCreateFromData.
 *
 * Wire conventions, derived from the IOHID ABI:
 *   - The IOHID event records (MoozHIDQueueElement / MoozHIDDigitizerEvent /
 *     MoozHIDVendorDefinedHeader) are emitted verbatim in host byte order,
 *     matching how CGEvent serializes its own structures.
 *   - The trailing gesture parameter fields use a big-endian tag/length/value
 *     framing: a 2-byte big-endian element count, a 1-byte value-type code, a
 *     1-byte field id, then the value(s) big-endian.
 */

/* Gesture-field value-type codes. */
enum {
    kMoozFieldTypeUInt32  = 0x40,
    kMoozFieldTypeFloat32 = 0xC0,
};

/* Magnify gesture subtype (NSEventType gesture subtype for pinch). */
enum { kMoozGestureSubtypeMagnify = 8 };

/* --- big-endian append primitives (endianness-independent by construction) --- */

static void moozAppendBE16(CFMutableDataRef data, uint16_t value) {
    const UInt8 bytes[2] = { (UInt8)(value >> 8), (UInt8)(value & 0xFF) };
    CFDataAppendBytes(data, bytes, sizeof bytes);
}

static void moozAppendBE32(CFMutableDataRef data, uint32_t value) {
    const UInt8 bytes[4] = {
        (UInt8)(value >> 24), (UInt8)(value >> 16),
        (UInt8)(value >> 8),  (UInt8)(value & 0xFF),
    };
    CFDataAppendBytes(data, bytes, sizeof bytes);
}

/* tag/length/value header: BE count, value-type code, field id. */
static void moozAppendFieldHeader(CFMutableDataRef data, uint8_t valueType,
                                  uint8_t field, uint16_t count) {
    moozAppendBE16(data, count);
    CFDataAppendBytes(data, &valueType, 1);
    CFDataAppendBytes(data, &field, 1);
}

static void moozAppendIntField(CFMutableDataRef data, uint8_t field, uint32_t value) {
    moozAppendFieldHeader(data, kMoozFieldTypeUInt32, field, 1);
    moozAppendBE32(data, value);
}

static void moozAppendFloatField(CFMutableDataRef data, uint8_t field, float value) {
    moozAppendFieldHeader(data, kMoozFieldTypeFloat32, field, 1);
    uint32_t bits;
    memcpy(&bits, &value, sizeof bits); /* reinterpret IEEE-754 bits */
    moozAppendBE32(data, bits);         /* emit big-endian */
}

/* mach_absolute_time converted to nanoseconds. */
static uint64_t moozNowNanos(void) {
    static mach_timebase_info_data_t timebase;
    if (timebase.denom == 0) {
        mach_timebase_info(&timebase);
    }
    return mach_absolute_time() * timebase.numer / timebase.denom;
}

/*
 * Builds the full magnify-gesture CGEvent for the given parameters and event
 * time. Returns a +1 retained CGEventRef (caller releases), or NULL on
 * failure. `timestamp` is a seam so callers — and the equivalence test — can
 * pin the event time; production passes the current host time.
 */
static CGEventRef moozBuildMagnifyEvent(double magnification, int32_t phase,
                                        uint64_t timestamp) {
    /* 1. Prototype gesture event: type 29 (NSEventTypeGesture), flags 256. */
    CGEventRef proto = CGEventCreate(NULL);
    if (!proto) {
        return NULL;
    }
    CGEventSetType(proto, (CGEventType)29);
    CGEventSetFlags(proto, (CGEventFlags)256);
    CGEventSetTimestamp(proto, timestamp);

    CFDataRef protoData = CGEventCreateData(kCFAllocatorDefault, proto);
    CFRelease(proto);
    if (!protoData) {
        return NULL;
    }

    CFMutableDataRef blob = CFDataCreateMutableCopy(kCFAllocatorDefault, 0, protoData);
    CFRelease(protoData);

    /* 2. Drop the 24 trailing bytes CGEvent appends for an empty gesture; the
     *    IOHID queue payload below takes their place. */
    CFIndex length = CFDataGetLength(blob);
    if (length >= 24) {
        CFDataDeleteBytes(blob, CFRangeMake(length - 24, 24));
    }

    const uint32_t vendorPayloadLength = 40;
    const uint32_t vendorDataSize =
        (uint32_t)sizeof(MoozHIDVendorDefinedHeader) + vendorPayloadLength;
    const uint16_t totalSize = (uint16_t)(sizeof(MoozHIDQueueElement) +
                                          vendorDataSize +
                                          sizeof(MoozHIDDigitizerEvent));

    /* 3. IOHID payload length + record tag. */
    moozAppendBE16(blob, totalSize);
    const UInt8 recordTag[2] = { 0x10, 0x6D };
    CFDataAppendBytes(blob, recordTag, sizeof recordTag);

    /* 4. Queue element collecting two records (parent digitizer + vendor). */
    MoozHIDQueueElement queue;
    memset(&queue, 0, sizeof queue);
    queue.timeStamp = timestamp;
    queue.options = kMoozHIDEventOptionIsCollection;
    queue.eventCount = 2;
    CFDataAppendBytes(blob, (const UInt8 *)&queue, sizeof queue);

    /* 5. Parent digitizer: a hand collection with no child touches. */
    MoozHIDDigitizerEvent parent;
    memset(&parent, 0, sizeof parent);
    parent.size = (uint32_t)sizeof parent;
    parent.type = kMoozHIDEventTypeDigitizer;
    parent.timestamp = timestamp;
    parent.options = kMoozHIDEventOptionIsCollection;
    parent.transducerType = kMoozHIDDigitizerTransducerHand;
    CFDataAppendBytes(blob, (const UInt8 *)&parent, sizeof parent);

    /* 6. Vendor-defined token followed by a zeroed 40-byte payload. */
    MoozHIDVendorDefinedHeader vendor;
    memset(&vendor, 0, sizeof vendor);
    vendor.size = vendorDataSize;
    vendor.type = kMoozHIDEventTypeVendorDefined;
    vendor.usagePage = 0xFF00;
    vendor.usage = 0x1777;
    vendor.version = 1;
    vendor.length = vendorPayloadLength;
    CFDataAppendBytes(blob, (const UInt8 *)&vendor, sizeof vendor);
    const UInt8 vendorPayload[40] = { 0 };
    CFDataAppendBytes(blob, vendorPayload, sizeof vendorPayload);

    /* 7. Gesture parameter fields (big-endian TLV). */
    moozAppendIntField(blob, 0x6E, kMoozGestureSubtypeMagnify);
    moozAppendIntField(blob, 0x6F, 0);
    moozAppendIntField(blob, 0x70, 0);
    moozAppendIntField(blob, 0x84, (uint32_t)phase);
    moozAppendIntField(blob, 0x85, 0);
    moozAppendFloatField(blob, 0x71, (float)magnification);
    moozAppendFloatField(blob, 0x8B, 0.0f);
    moozAppendFloatField(blob, 0x8C, 0.0f);

    /* 8. Materialize the event from the assembled blob. */
    CGEventRef event = CGEventCreateFromData(kCFAllocatorDefault, blob);
    CFRelease(blob);
    return event;
}

void MoozEmitMagnify(double magnification, int32_t phase) {
    CGEventRef event = moozBuildMagnifyEvent(magnification, phase, moozNowNanos());
    if (event) {
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
    }
}
