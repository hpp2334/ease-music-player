# Motivation

## 0.1 -> 0.2: switch from Flutter to Jetpack Compose

[In version 0.1](https://github.com/hpp2334/ease-music-player/tree/feat/flutter), the application was implemented using Rust + Flutter. After some time of use, several issues were discovered, such as:

- It is hard to call flutter from native synchronously.

- Flutter's complexity in cross-platform development, especially when dealing with platform-specific features.

- Difficulty in finding solutions for some minor issues, such as the absence of system-provided crash dialogs when the program crashes.

- Redundancy in "cross-platform" capabilities when compared to Rust.

- ...

Due to the reasons above, the current version uses Jetpack Compose to better integrate with the Android platform.

## 0.2 -> 0.3: remove view models in Rust

It is challenging to implement complex interactions and animations using view models in Rust, primarily because the core UI is built with Jetpack Compose. In addition, after some preliminary experimentation with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui) and other GUI frameworks, it became apparent that achieving cross-platform compatibility between Android and Linux is particularly difficult.

As a result, all view models have been removed from the Rust codebase. Rust is now used solely as the backend language.
