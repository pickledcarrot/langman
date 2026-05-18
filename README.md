# Langman

Langman is a command-line language practice app. It currently supports Spanish
and generates short fill-in-the-blank drills for CEFR levels A1 through C2.

The exercises are generated with the OpenAI API and are intended to practice
grammar, conjugations, and vocabulary. The app also includes a built-in
Spanish grammar review sheet so you can study rules alongside the drills. It
now uses SQLite for local persistence.

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

Langman stores local learning data in:

```sh
data/langman.db
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
4. Offer an optional Spanish grammar review before the drill.
5. Generate five fill-in-the-blank exercises.
6. Prompt you for each answer, then show the explanation whether you were correct or not.
7. Show your final score.

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
- Built-in Spanish grammar review notes in `resources/spanish_grammar.txt`
- SQLite persistence for generated exercises, study sessions, and attempts

## Database Model

The initial SQLite schema is in `resources/schema.sql`.

- `grammar_rules`: canonical grammar concepts, examples, mistakes, and tags
- `study_sessions`: each drill or future flashcard session
- `exercises`: reusable exercise bank entries linked to a canonical `grammar_rule_id`
- `session_exercises`: which exercises were used in a given session
- `exercise_attempts`: the learner's answers, correctness, explanation snapshot, and grammar rule link

This keeps the current CLI simple while leaving a clean path to future features
like flashcards, saved review queues, and per-topic progress tracking.

Spanish grammar rules are seeded from `resources/spanish_grammar_rules.json`. When
the app generates exercises, it now requires each item to reference one of those
canonical rule IDs so progress can be tracked by actual grammar topic.
