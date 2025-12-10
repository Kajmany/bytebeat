# Bytebeat: What if another Bytebeat composer, but a desktop application?

In fact, several good reasons exist for not doing so, but I ignored them.

# Requirements
## Running
- A system running Pipewire for audio, which is most modern-ish Linux distributions.
- A crossterm-compatible terminal, which you probably have. See: https://github.com/crossterm-rs/crossterm#tested-terminals
- A token amount of RAM, CPU time, etc.
## Building/Testing
- GCC or Clang for the hacky build suite in `src/parser`, so that we can compare samples generated with C
- ???: Clang and Pipewire development headers so that the audio backend can be built, because it's a wrapper with generated bindings
    - TODO: Verify this!

# Features
- Responsive TUI: Cross-platform, reasonably responsive (thanks event-loop template!)
- Audio Backend: Traditional 8000Hz U8 samples sent via Pipewire only, using the Stream API. Resampling is outsourced to Pipewire.
- Basic code entry: Single line, with current-character highlighting and the ability to jump cursor to whitespace boundaries.
- Dynamic code evaluation: Supports C-syntax and operators needed for classical bytebeat codes. 't' and intermediates are i32 while output wraps around to u8.
    - Single statement and multi-expression, no semicolons. No newlines (for now!)
    - Arithmetic: +, -, *, /, %
    - Logical: &&, ||, !
    - Bitwise: &, |, ^, ~, <<, >>
    - Comparison: ==, !=, <, <=, >, >=
    - Ternary: ? :
    - Variable: Just 't' for time!
    - Ordering: (, )
- True-to-C evaluation: According to my system's compiler, because the tests compare samples to those generated in C with the same bytebeat codes. I might be missing edge cases, but every operator is represented at least once in testing.
- Lexer/Parser Recovery & Positionally-aware Errors: Attempts to deliver all errors and their column occurance at once upon failed compilation.
- Logging: Optional logging to file with `tracing`, which will spawn a separate thread to minimize blocking on the main (render) thread.
- Audio control: Currently just Play/Pause. Volume control is another TODO.

# TODO
- Volume control. Should use a small TUI widget and send commands to the audio thread.
- Logging
    - Let CLI specify verbosity
    - Display logs in TUI using widget
- Operating modes
    - Headless (maybe?) - allow codes to be piped
    - File-watch (definitely) so we don't need a more complicated editor/errors. Will need multiline support. Should be tested so that common IDE's can work in the file.
- Audio Backends
    - Windows via WASAPI OR Rust cpal (Probably)
    - MacOS via ??? (Probably not)
- Wave Visualizer! Real feather in the hat. Can use Ratatui Canvas and share a ring buffer with audio thread.
- Popup modal & toggleable focus
    - Help section (probably not in favor of README)
    - Song library?
    - Bigger logging display
    - About? could substitute for help by linking back to README.

# AI Usage
Source modules which approach cognitohazard level of LLM usage are marked with a doc comment, but otherwise mostly everywhere to varying extents. This README, all doc-comments, and most code comments are entirely from the heart and written with human intention.

Architectural foundations are my own work and/or based on example/starter code from Pipewire-rs and Ratatui, so I don't personally consider the codebase to be drowning in incomprehensible slop. The current generation of LLMs are especially want to misuse abstractions and create redundant ones, so I've taken extra care to try to avoid allowing that to bloat the codebase.