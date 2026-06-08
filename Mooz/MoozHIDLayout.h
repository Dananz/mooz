#ifndef MoozHIDLayout_h
#define MoozHIDLayout_h

#include <stdint.h>

/*
 * MoozHIDLayout.h
 *
 * Minimal, original definitions of the Apple IOHID event records that make up
 * a serialized trackpad-gesture CGEvent. These structs describe Apple's binary
 * wire layout (an interoperability fact: field order, sizes, and offsets); the
 * code that fills and serializes them lives in MoozGesture.m and is original to
 * Mooz. No third-party source is reproduced here.
 *
 * The records are written to the blob in host byte order (CGEvent's own
 * convention), so the field types below mirror the native widths exactly.
 */

/* IOHID event-record type codes (Apple ABI ordinals). */
enum {
    kMoozHIDEventTypeVendorDefined = 1,
    kMoozHIDEventTypeDigitizer     = 11,
};

/* IOHID event option bits. The collection bit marks a record that groups
 * child events (here: the hand/digitizer parent). */
enum {
    kMoozHIDEventOptionIsCollection = 0x00000002,
};

/* Digitizer transducer kinds. A whole-hand gesture uses "hand". */
enum {
    kMoozHIDDigitizerTransducerHand = 0x23,
};

/*
 * Vendor-defined event record. The 20-byte common event head (size, type,
 * timestamp, options) is laid out inline so offsets are unambiguous; the
 * struct pads to an 8-byte boundary, giving a 32-byte header. `length` payload
 * bytes follow the header in the blob.
 */
typedef struct {
    uint32_t size;       /* total record size incl. payload          (off 0)  */
    uint32_t type;       /* kMoozHIDEventTypeVendorDefined            (off 4)  */
    uint64_t timestamp;  /* host event time, nanoseconds              (off 8)  */
    uint32_t options;    /* IOHID option bits                         (off 16) */
    uint16_t usagePage;  /* HID usage page                            (off 20) */
    uint16_t usage;      /* HID usage                                 (off 22) */
    uint32_t version;    /* token version                            (off 24) */
    uint32_t length;     /* trailing payload length, bytes            (off 28) */
    /* payload bytes follow (header size == 32) */
} MoozHIDVendorDefinedHeader;

/* Three-axis 16.16 fixed-point position; zero for a pinch/zoom gesture. */
typedef struct {
    int32_t x; /* off 0 */
    int32_t y; /* off 4 */
    int32_t z; /* off 8 */
} MoozHIDPosition;

/*
 * Digitizer event record. Used here as the parent collection of a hand
 * gesture. Every field except size/type/timestamp/options/transducerType is
 * zero for a touch-free zoom. The trailing five-element array is the widest
 * arm of Apple's orientation union; it pads the record to 96 bytes.
 */
typedef struct {
    uint32_t        size;            /* sizeof(record)                (off 0)  */
    uint32_t        type;            /* kMoozHIDEventTypeDigitizer     (off 4)  */
    uint64_t        timestamp;       /* host event time, nanoseconds   (off 8)  */
    uint32_t        options;         /* IOHID option bits              (off 16) */
    MoozHIDPosition position;        /* averaged child position        (off 20) */
    uint32_t        transducerIndex; /* (off 32) */
    uint32_t        transducerType;  /* kMoozHIDDigitizerTransducerHand (off 36) */
    uint32_t        identity;        /* (off 40) */
    uint32_t        eventMask;       /* (off 44) */
    uint32_t        childEventMask;  /* (off 48) */
    uint32_t        buttonMask;      /* (off 52) */
    int32_t         tipPressure;     /* (off 56) */
    int32_t         barrelPressure;  /* (off 60) */
    int32_t         twist;           /* (off 64) */
    uint32_t        orientationType; /* (off 68) */
    int32_t         orientation[5];  /* widest orientation arm         (off 72) */
} MoozHIDDigitizerEvent; /* sizeof == 96 */

/*
 * One element of the IOHID system event queue: a 24-byte header followed by a
 * run of event records. `eventCount` counts the records that follow.
 */
typedef struct {
    uint64_t timeStamp;  /* host event time, nanoseconds   (off 0)  */
    uint64_t deviceID;   /* sending device id              (off 8)  */
    uint32_t options;    /* IOHID option bits              (off 16) */
    uint32_t eventCount; /* number of trailing records     (off 20) */
    /* event records follow (header size == 24) */
} MoozHIDQueueElement;

#endif /* MoozHIDLayout_h */
