mod db;

use db::{AttemptRecord, Database, GrammarRule, SavedSession};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::error::Error;
use std::io::{self, Write};

const LEVELS: [&str; 6] = ["A1", "A2", "B1", "B2", "C1", "C2"];
const SPANISH_GRAMMAR_GUIDE: &str = include_str!("../resources/spanish_grammar.txt");

#[derive(Debug, Deserialize)]
struct ExerciseSet {
    exercises: Vec<Exercise>,
}

#[derive(Debug, Deserialize)]
pub struct Exercise {
    focus_rule_id: String,
    sentence: String,
    answer: String,
    hint: String,
    focus: String,
    explanation: String,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: Vec<InputText<'a>>,
}

#[derive(Debug, Serialize)]
struct InputText<'a> {
    #[serde(rename = "type")]
    content_type: &'a str,
    text: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("\nError: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    println!("Langman");
    println!("Practice languages with short fill-in-the-blank drills.\n");

    let database = Database::new()?;
    println!("Database: {}", database.path().display());

    let language = select_language()?;
    let level = select_level()?;
    maybe_review_grammar(language)?;

    println!("\nGenerating {language} exercises for level {level}...");
    let grammar_rules = database.grammar_rules_for_language(language)?;
    let exercises = generate_exercises(language, level, &grammar_rules)?;
    let model = current_model();
    let saved_session = database.start_generated_session(language, level, &model, &exercises)?;
    println!(
        "Saved {} exercises to the exercise bank.",
        saved_session.saved_exercises.len()
    );
    run_drill(&database, language, level, &saved_session, &exercises)?;

    Ok(())
}

fn select_language() -> Result<&'static str, Box<dyn Error>> {
    println!("Choose a language:");
    println!("1. Spanish");

    loop {
        let choice = prompt("Selection: ")?;
        match choice.trim() {
            "1" => return Ok("Spanish"),
            _ => println!("Please enter 1."),
        }
    }
}

fn select_level() -> Result<&'static str, Box<dyn Error>> {
    println!("\nChoose your level:");
    for (index, level) in LEVELS.iter().enumerate() {
        println!("{}. {level}", index + 1);
    }

    loop {
        let choice = prompt("Selection: ")?;
        match choice.trim().parse::<usize>() {
            Ok(number) if (1..=LEVELS.len()).contains(&number) => return Ok(LEVELS[number - 1]),
            _ => println!("Please enter a number from 1 to {}.", LEVELS.len()),
        }
    }
}

fn run_drill(
    database: &Database,
    language: &str,
    level: &str,
    saved_session: &SavedSession,
    exercises: &[Exercise],
) -> Result<(), Box<dyn Error>> {
    println!("\n{language} {level} drill");
    println!("Type the missing word or phrase. Press Enter to submit.\n");

    let mut correct_count = 0;

    for (index, exercise) in exercises.iter().enumerate() {
        let saved_exercise = saved_session.saved_exercises.get(index).ok_or_else(|| {
            format!(
                "Exercise {} was not saved correctly before the drill started.",
                index + 1
            )
        })?;

        println!("{}. {}", index + 1, exercise.sentence);
        println!("Focus: {}", exercise.focus);
        println!("Hint: {}", exercise.hint);

        let answer = prompt("Your answer: ")?;
        let is_correct = normalize_answer(&answer) == normalize_answer(&exercise.answer);
        if is_correct {
            correct_count += 1;
            println!("Correct.");
            println!("Explanation: {}\n", exercise.explanation);
        } else {
            println!("Answer: {}", exercise.answer);
            println!("Explanation: {}\n", exercise.explanation);
        }

        let attempt = AttemptRecord {
            exercise_id: &saved_exercise.id,
            prompt: &saved_exercise.prompt,
            user_answer: answer.trim(),
            accepted_answer: &exercise.answer,
            is_correct,
            explanation: &exercise.explanation,
            focus: &exercise.focus,
            grammar_rule_id: &saved_exercise.grammar_rule_id,
            attempt_index: index + 1,
        };
        database.record_attempt(&saved_session.session_id, &attempt)?;
    }

    println!("Score: {correct_count}/{}", exercises.len());
    Ok(())
}

fn maybe_review_grammar(language: &str) -> Result<(), Box<dyn Error>> {
    if language != "Spanish" {
        return Ok(());
    }

    println!("\nReview essential Spanish grammar before the drill?");
    println!("1. Yes");
    println!("2. No");

    loop {
        let choice = prompt("Selection: ")?;
        match choice.trim() {
            "1" => {
                println!("\n{SPANISH_GRAMMAR_GUIDE}");
                wait_for_enter("\nPress Enter to continue to the drill setup...")?;
                return Ok(());
            }
            "2" => return Ok(()),
            _ => println!("Please enter 1 or 2."),
        }
    }
}

fn generate_exercises(
    language: &str,
    level: &str,
    grammar_rules: &[GrammarRule],
) -> Result<Vec<Exercise>, Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
        "OPENAI_API_KEY is not set. Export it before running langman, for example: export OPENAI_API_KEY=..."
    })?;
    let model = current_model();
    let grammar_rule_prompt = grammar_rule_prompt(grammar_rules);

    let request_body = json!({
        "model": model,
        "input": [
            Message {
                role: "developer",
                content: vec![InputText {
                    content_type: "input_text",
                    text: format!("You are a careful language tutor. Generate short, level-appropriate fill-in-the-blank practice items. Use exactly one blank marker: ____. The blank should test grammar, conjugation, or vocabulary. Keep answers unambiguous. Every exercise must use one grammar rule from the approved rule list below and must set focus_rule_id to that exact rule id.\n\nApproved grammar rules:\n{grammar_rule_prompt}\n\nReturn only structured data that matches the schema."),
                }],
            },
            Message {
                role: "user",
                content: vec![InputText {
                    content_type: "input_text",
                    text: format!("Create 5 fill-in-the-blank exercises for a {level} learner of {language}. The learner should type the missing word or short phrase. Include a brief hint, the grammar/vocabulary focus title, the matching focus_rule_id, and a short explanation."),
                }],
            }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "language_drill",
                "strict": true,
                "schema": exercise_schema()
            }
        }
    });

    let response = Client::new()
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()?;

    let status = response.status();
    let response_body = response.text()?;
    if !status.is_success() {
        return Err(format!("OpenAI API request failed with {status}: {response_body}").into());
    }

    let response: Value = serde_json::from_str(&response_body)?;

    let output_text = extract_output_text(&response)
        .ok_or("OpenAI response did not include output text with exercises.")?;
    let exercise_set: ExerciseSet = serde_json::from_str(&output_text)?;

    if exercise_set.exercises.is_empty() {
        return Err("OpenAI returned no exercises.".into());
    }

    validate_exercises(&exercise_set.exercises, grammar_rules)?;
    Ok(exercise_set.exercises)
}

fn validate_exercises(
    exercises: &[Exercise],
    grammar_rules: &[GrammarRule],
) -> Result<(), Box<dyn Error>> {
    if exercises.len() != 5 {
        return Err(format!(
            "OpenAI returned {} exercises instead of 5.",
            exercises.len()
        )
        .into());
    }

    let valid_rule_ids: std::collections::HashSet<&str> =
        grammar_rules.iter().map(|rule| rule.id.as_str()).collect();

    for (index, exercise) in exercises.iter().enumerate() {
        if exercise.sentence.matches("____").count() != 1 {
            return Err(format!(
                "Exercise {} does not contain exactly one blank marker.",
                index + 1
            )
            .into());
        }

        if exercise.answer.trim().is_empty() {
            return Err(format!("Exercise {} has an empty answer.", index + 1).into());
        }

        if !valid_rule_ids.contains(exercise.focus_rule_id.as_str()) {
            return Err(format!(
                "Exercise {} references unknown focus_rule_id '{}'.",
                index + 1,
                exercise.focus_rule_id
            )
            .into());
        }
    }

    Ok(())
}

fn exercise_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["exercises"],
        "properties": {
            "exercises": {
                "type": "array",
                "minItems": 5,
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["focus_rule_id", "sentence", "answer", "hint", "focus", "explanation"],
                    "properties": {
                        "focus_rule_id": {
                            "type": "string",
                            "description": "The exact grammar rule id from the approved rule list."
                        },
                        "sentence": {
                            "type": "string",
                            "description": "A sentence containing exactly one ____ blank marker."
                        },
                        "answer": {
                            "type": "string",
                            "description": "The exact missing word or short phrase."
                        },
                        "hint": {
                            "type": "string",
                            "description": "A short learner-facing clue."
                        },
                        "focus": {
                            "type": "string",
                            "description": "The grammar, conjugation, or vocabulary topic being practiced."
                        },
                        "explanation": {
                            "type": "string",
                            "description": "A concise explanation of why the answer is correct."
                        }
                    }
                }
            }
        }
    })
}

fn extract_output_text(response: &Value) -> Option<String> {
    response
        .get("output")?
        .as_array()?
        .iter()
        .filter_map(|item| item.get("content")?.as_array())
        .flat_map(|content| content.iter())
        .find_map(|content_item| {
            let content_type = content_item.get("type")?.as_str()?;
            if content_type == "output_text" {
                content_item.get("text")?.as_str().map(str::to_string)
            } else {
                None
            }
        })
}

fn normalize_answer(answer: &str) -> String {
    answer.trim().to_lowercase()
}

fn current_model() -> String {
    env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-mini".to_string())
}

fn grammar_rule_prompt(grammar_rules: &[GrammarRule]) -> String {
    grammar_rules
        .iter()
        .map(|rule| {
            format!(
                "- {}: {} ({}, CEFR {})\n  Rule: {}\n  Examples: {}",
                rule.id,
                rule.title,
                rule.category,
                rule.cefr_level,
                rule.rule_text,
                rule.examples.join(" | ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{label}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}

fn wait_for_enter(label: &str) -> Result<(), Box<dyn Error>> {
    let _ = prompt(label)?;
    Ok(())
}
