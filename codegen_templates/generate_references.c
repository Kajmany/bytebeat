// Used only during testing to generate samples for the parser component
// We're at the mercy of the system's C compiler but this seems to be a better
// tradeoff than trying to translate these into Rust syntax and hoping we're not
// run-over by any quirks.

// Included songs are copyright their respective owners.
#include <stdint.h>

// These should be presented and evaluated in their original form (matches our
// Rust tests, too)
#pragma clang diagnostic ignored "-Wbitwise-conditional-parentheses"
#pragma clang diagnostic ignored "-Wparentheses"

typedef uint8_t beat_function(int t);
extern beat_function *songs[];
uint8_t generate_sample(int32_t song_idx, int32_t t) {
  return songs[song_idx](t);
}

// build.rs will add jump table with a function for each song below.