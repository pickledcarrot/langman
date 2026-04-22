use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::error::Error;
use std::io::{self, Write};

const LEVELS: [&str; 6] = ["A1", "A2", "B1", "B2", "C1", "C2"];

#[derive(Debug, Deserialize)]
struct ExerciseSet {
    exercises: Vec<Exercise>,
}

#[derive(Debug, Deserialize)]
struct Exercise {
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

    let language = select_language()?;
    let level = select_level()?;

    println!("\nGenerating {language} exercises for level {level}...");
    let exercises = generate_exercises(language, level)?;
    run_drill(language, level, &exercises)?;

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

fn run_drill(language: &str, level: &str, exercises: &[Exercise]) -> Result<(), Box<dyn Error>> {
    println!("\n{language} {level} drill");
    println!("Type the missing word or phrase. Press Enter to submit.\n");

    let mut correct_count = 0;

    for (index, exercise) in exercises.iter().enumerate() {
        println!("{}. {}", index + 1, exercise.sentence);
        println!("Focus: {}", exercise.focus);
        println!("Hint: {}", exercise.hint);

        let answer = prompt("Your answer: ")?;
        if normalize_answer(&answer) == normalize_answer(&exercise.answer) {
            correct_count += 1;
            println!("Correct.\n");
        } else {
            println!("Answer: {}", exercise.answer);
            println!("Explanation: {}\n", exercise.explanation);
        }
    }

    println!("Score: {correct_count}/{}", exercises.len());
    Ok(())
}

fn generate_exercises(language: &str, level: &str) -> Result<Vec<Exercise>, Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
        "OPENAI_API_KEY is not set. Export it before running langman, for example: export OPENAI_API_KEY=..."
    })?;
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-mini".to_string());

    let request_body = json!({
        "model": model,
        "input": [
            Message {
                role: "developer",
                content: vec![InputText {
                    content_type: "input_text",
                    text: "You are a careful language tutor. Generate short, level-appropriate fill-in-the-blank practice items. Use exactly one blank marker: ____. The blank should test grammar, conjugation, or vocabulary. Keep answers unambiguous. Return only structured data that matches the schema.".to_string(),
                }],
            },
            Message {
                role: "user",
                content: vec![InputText {
                    content_type: "input_text",
                    text: format!("Create 5 fill-in-the-blank exercises for a {level} learner of {language}. The learner should type the missing word or short phrase. Include a brief hint, the grammar/vocabulary focus, and a short explanation."),
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

    validate_exercises(&exercise_set.exercises)?;
    Ok(exercise_set.exercises)
}

fn validate_exercises(exercises: &[Exercise]) -> Result<(), Box<dyn Error>> {
    if exercises.len() != 5 {
        return Err(format!(
            "OpenAI returned {} exercises instead of 5.",
            exercises.len()
        )
        .into());
    }

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
                    "required": ["sentence", "answer", "hint", "focus", "explanation"],
                    "properties": {
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

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{label}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}
