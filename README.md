# Bytebeat: What if another Bytebeat composer, but a desktop application?

In fact, several good reasons exist for not doing so, but I ignored them.

# Requirements
## Running
- A system that can handle one of two audio backends:
    - Pipewire, which is now running on most modern-ish Linux distributions.
    - WASAPI, which requires only Vista or later, but the Rust targets supposedly require at least Windows 10.
- A crossterm-compatible terminal, which you probably have. See: https://github.com/crossterm-rs/crossterm#tested-terminals
- A token amount of RAM, CPU time, etc.
## Building/Testing
- GCC or Clang for the hacky build suite in `src/parser`, so that we can compare samples generated with C
- ???: Clang and Pipewire development headers so that the audio backend can be built, because it's a wrapper with generated bindings
    - TODO: Verify this!

# Features
- Wow, Cool TUI(?): Cross-platform, reasonably responsive (thanks event-loop template!)
- Song Library: Play a hard-coded library of most classic C-compatible Dollchan songs. See evaluation limitations for what's excluded.
- Wave Visualizer: Like the [DollChan scope](https://github.com/SthephanShinkufag/bytebeat-composer), but worse. They aren't limited to braille characters, in my defense.
- Audio Backend: Traditional 8KHz u8 samples sent via Pipewire or WASAPI. Resampling is handled by the audio server and not this application.
- Inputs: Interactive and file-watching.
    - Interactive: `-i` and default/implicit. Really simple single line input. You can see the cursor and jump word boundaries.
    - File-watching: `-f` Uses [notify-rs'](https://github.com/notify-rs/notify) cross-platform bag of tricks to reload a beat from a single file. Stdin is still used for controls.
- Dynamic code evaluation: Supports C-syntax and operators needed for classical bytebeat codes. 't' and intermediates are i32 while output wraps around to u8.
    - Single statement and multi-expression, no semicolons.
    - Arithmetic: `+ - * / %`
    - Logical: `&& || !`
    - Bitwise: `& | ^ ~ << >>`
    - Comparison: `== != < <= > >=`
    - Ternary: `? :`
    - Variable: Just `t` for time!
    - Ordering: `( )`
    - Numbers: Bases 2, 8, 10, 16 with C-prefixes `0b10101` `0407` `1337` `0xDEADBEEF`
    - NOT SUPPORTED: Array creation, floats (as input OR output), functions (including math.h)...
- True-to-C evaluation: According to my system's compiler, because the tests compare samples to those generated in C with the same bytebeat codes. I might be missing edge cases, but every operator is represented at least once in testing.
- Lexer/Parser Recovery & Positionally-aware Errors: Attempts to deliver all errors and their column occurance at once upon failed compilation.
- Logging: Most recent logs in-TUI, optional file logging (*may* provide path) `-l`, verbosity configurable with `RUST_LOG` or `-v --verbose`. Environment variable has precedence over flag.
- Audio control: Play/Pause the stream, and volume is controllable 0%-100%. Backend handles the audio control -- you don't want to hear what it sounds like if we multiply 8-bit samples by an f32.

## TUI Views
- Main: You start here. There's a scope, small log, input bar, status bar, and controls at the bottom. Pound Esc like a brute to always return here.
- Help Modal: May be displayed over any view. Has controls and some terse advice. Press F1 to toggle.
- Big Log: Takes up the scope and small log. (TODO)
- Library: Also takes up the scope and small log. Paginated song table allows selecting from hardcoded songs. You may just 'sample' within the menu or over-write the buffer to 'take' the song out of just this menu.

# TODO
- Make song library and table less slopped
- Filtering for song library
- Test which tries to parse every song in library (some fail)
- Bigger logging display
- Finish File-watcher
- MacOS Audio Backend (Core Audio?): It'd be neat to compare a third audio API, but I don't have a device to test this on and my willpower is drained after doing two.
- HD Wave Visualizer: Kind of ridiculous, but it's theoretically possible on Sixel/Kitty Graphics supporting terminals to pack our own image buffers and draw the visualizer with them. Might perform too slowly to be viable.

# License & Song Provenance
Dubious. This repository contains songs/'codes' which I do not own the copyright to. Where known, the composers are credited. These are present in:

- `library.csv`
- `src/library_data.rs` (when built)
- `src/parser/generate_references.c`
- `src/parser.rs` (test module)

All of these are widely available and this list is scraped from the [dollchan composer library](https://github.com/SthephanShinkufag/bytebeat-composer/tree/master/data/library) classic archive.

# LLM Usage
Source modules which approach cognitohazard level of LLM usage are marked with a doc comment, but otherwise mostly everywhere to varying extents. This README, all doc-comments, and most code comments are entirely from the heart and written with human intention.

Architectural foundations are my own work and/or based on example/starter code from Pipewire-rs and Ratatui, so I don't personally consider the codebase to be drowning in incomprehensible slop. The current generation of LLMs are especially want to misuse abstractions and create redundant ones, so I've taken extra care to try to avoid allowing that to bloat the codebase.