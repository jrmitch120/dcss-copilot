/**
 * @file
 * @brief Various definitions used throughout.
 * (trimmed for the extractor test)
**/

#pragma once

#define X_BOUND_1               (-1 + BOUNDARY_BORDER)
#define X_WIDTH                 (X_BOUND_2 - X_BOUND_1 + 1)
#define LOS_RADIUS 8
#define LOS_DEFAULT_RANGE 7
#define AUTOMATIC_HIT           1500
#define POLAR_VORTEX_RADIUS 5

#if defined(TARGET_CPU_X86) && !defined(__SSE__)
#define FP_FAST_FMA
#endif

// colours, such pretty colours ...
// The order is important (IRGB bit patterns).
enum COLOURS
{
    COLOUR_INHERIT = -1,
    BLACK,
    COLOUR_UNDEF = BLACK,
    BLUE,
    GREEN,
    CYAN,
    RED,
    MAGENTA,
    BROWN,
    LIGHTGRAY,
    LIGHTGREY = LIGHTGRAY,
    DARKGRAY,
    DARKGREY = DARKGRAY,
    LIGHTBLUE,
    LIGHTGREEN,
    LIGHTCYAN,
    LIGHTRED,
    LIGHTMAGENTA,
    YELLOW,
    WHITE,
    MAX_TERM_COLOUR
};
