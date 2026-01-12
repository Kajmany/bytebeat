# Manually-Written Project Overview for AI Agents and Inquiring Minds

## Building
Use `cargo build`. Pipewire development headers such as via `pipewire-devel` (Fedora) are required to build on Linux.

## Testing
Components that aren't immediately interactive should be tested. Write test code at the end of the module following Rust test conventions. There's currently no automated UI testing. The parser tests currently require `gcc` or `clang` on `$PATH` to compile C test expressions for comparison with our output. Use `cargo test` after changing code.

## Coding Style
All code should be free of warnings or errors and formatted. Where you are absolutely sure that warnings are justified to create good code, use a clippy directive to hide the warning for the smallest possible scope. Use `cargo clippy` and `cargo rustfmt`

Follow these rules when creating or refactoring code:
- Use the shortest `use` import that is not ambiguous inside the module.
- Only use `pub` where necessary to expose an API for other modules.


## Commenting Style
DO NOT create documentation comments `///` or `//!` for any item. These are only for humans to make.
Create normal code comments `//` to explain complicated logic or decisions that appear non-trivial. Do not refactor or remove existing comments unless they contradict the code. Avoid using comments to talk to the user or think.

## Commit Style
Agents should not write commit messages. If they do anyway, then the agent SHOULD use the "Conventional Commit" standard and MUST add a "Co-authored-by: [AGENT NAME] [AGENT EMAIL (optional)]" trailer to the commit.

---

## Cross-Thread State
The `App` and the audio backend are the two major pieces of this program. State should usually flow between these two as messages brokered by the `EventHandler` struct in `event.rs`. 

App -> EventHandler -> Audio Backend
- App sends `AudioCommand` (pause, new beat, etc) to Audio Backend via mpsc.
- Audio Backend checks mpsc queue during a callback for commands.

App <- EventHandler <- Audio Backend
- Audio Backend uses mpsc to send `AudioEvent` back to App to inform of state changes.

App <- Audio Backend 
- Audio Backend sends all audio samples created to an `rtrb::RingBuffer` that the `Scope` widget uses to visualize.
- Audio Backend updates atomic `T_PLAY` as an estimation of which sample is thought to be playing, so `Scope` may track it.

App <- EventHandler <- EventThread
- Ticks are sent at fixed rate `TICK_FPS` which implicitly trigger an update in app, and may change widget state since it's propagated to them.
- Crossterm events are forwarded from the crossterm poll.
- File watch events are forwarded from the `notify::Reciever` when the user requests file watch input at startup.

## UI/App State
The `App` recieves messages from a queue in the `update` method. Some actions such as global keys are handled within `app.rs` from this method. Other messages are delegated to components stored in `src/app/`. Only `AppEvent` may mutate `App` state. Actions which need to mutate state enqueue an `AppEvent` which is processed *after* the next render.

The `App` will call public methods on components in response to app-events to mutate their state. 