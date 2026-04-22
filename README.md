# Langman

Langman is a command-line language practice app. It currently supports Spanish
and generates short fill-in-the-blank drills for CEFR levels A1 through C2.

The exercises are generated with the OpenAI API and are intended to practice
grammar, conjugations, and vocabulary.

## Requirements

- Rust
- An OpenAI API key

## Setup

Set your OpenAI API key before running the app:

```sh
export OPENAI_API_KEY=your_api_key_here
```

By default, Langman uses `gpt-5-mini`. You can override the model with:

```sh
export OPENAI_MODEL=gpt-5-mini
```

## Run

From the project directory:

```sh
cargo run
```

After starting, Langman will:

1. Show a language menu.
2. Let you choose Spanish.
3. Ask for your level: A1, A2, B1, B2, C1, or C2.
4. Generate five fill-in-the-blank exercises.
5. Prompt you for each answer and show your final score.

## Build

```sh
cargo build --release
```

The release binary will be available at:

```sh
target/release/langman
```

If `target/release` is on your `PATH`, you can run the app with:

```sh
langman
```

## Current Scope

- Spanish only
- Terminal menus only
- Five generated exercises per session
- Exact answer matching after trimming and lowercasing

