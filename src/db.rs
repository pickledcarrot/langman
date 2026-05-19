use crate::Exercise;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_DB_PATH: &str = "data/langman.db";
const SCHEMA: &str = include_str!("../resources/schema.sql");
const SPANISH_GRAMMAR_RULES: &str = include_str!("../resources/spanish_grammar_rules.json");

pub struct Database {
    db_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrammarRule {
    pub id: String,
    pub language: String,
    pub cefr_level: String,
    pub category: String,
    pub title: String,
    pub rule_text: String,
    pub examples: Vec<String>,
    pub common_mistakes: Vec<String>,
    pub tags: Vec<String>,
}

pub struct SavedSession {
    pub session_id: String,
    pub saved_exercises: Vec<SavedExercise>,
}

pub struct SavedExercise {
    pub id: String,
    pub prompt: String,
    pub grammar_rule_id: String,
}

pub struct RecentExercise {
    pub prompt: String,
    pub answer: String,
}

pub struct AttemptRecord<'a> {
    pub exercise_id: &'a str,
    pub grammar_rule_id: &'a str,
    pub prompt: &'a str,
    pub user_answer: &'a str,
    pub accepted_answer: &'a str,
    pub is_correct: bool,
    pub explanation: &'a str,
    pub focus: &'a str,
    pub attempt_index: usize,
}

impl Database {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let db_path = PathBuf::from(DEFAULT_DB_PATH);
        ensure_parent_dir(&db_path)?;

        let database = Self { db_path };
        database.initialize()?;
        Ok(database)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    pub fn grammar_rules_for_language(
        &self,
        language: &str,
    ) -> Result<Vec<GrammarRule>, Box<dyn Error>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, language, COALESCE(cefr_level, ''), category, title, rule_text,
                    examples_json, common_mistakes_json, tags_json
             FROM grammar_rules
             WHERE language = ?1
             ORDER BY title",
        )?;

        let rows = statement.query_map(params![language], |row| {
            Ok(GrammarRule {
                id: row.get(0)?,
                language: row.get(1)?,
                cefr_level: row.get(2)?,
                category: row.get(3)?,
                title: row.get(4)?,
                rule_text: row.get(5)?,
                examples: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(6)?)
                    .unwrap_or_default(),
                common_mistakes: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(7)?)
                    .unwrap_or_default(),
                tags: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(8)?)
                    .unwrap_or_default(),
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }

        Ok(rules)
    }

    pub fn start_generated_session(
        &self,
        language: &str,
        level: &str,
        model: &str,
        exercises: &[Exercise],
    ) -> Result<SavedSession, Box<dyn Error>> {
        let session_id = generate_id("session");
        let created_at = unix_timestamp();
        let connection = self.open()?;

        connection.execute(
            "INSERT INTO study_sessions (
                id, language, cefr_level, session_type, source_model, created_at
            ) VALUES (?1, ?2, ?3, 'generated_drill', ?4, ?5)",
            params![session_id, language, level, model, created_at],
        )?;

        let mut saved_exercises = Vec::with_capacity(exercises.len());
        for exercise in exercises {
            let exercise_id =
                self.insert_or_get_exercise(&connection, language, level, exercise)?;
            connection.execute(
                "INSERT INTO session_exercises (session_id, exercise_id) VALUES (?1, ?2)",
                params![session_id, exercise_id],
            )?;

            saved_exercises.push(SavedExercise {
                id: exercise_id,
                prompt: exercise.sentence.clone(),
                grammar_rule_id: exercise.focus_rule_id.clone(),
            });
        }

        Ok(SavedSession {
            session_id,
            saved_exercises,
        })
    }

    pub fn recent_exercises(
        &self,
        language: &str,
        level: &str,
        limit: usize,
    ) -> Result<Vec<RecentExercise>, Box<dyn Error>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT prompt, answer
             FROM exercises
             WHERE language = ?1 AND cefr_level = ?2
             ORDER BY created_at DESC
             LIMIT ?3",
        )?;

        let rows = statement.query_map(params![language, level, limit as i64], |row| {
            Ok(RecentExercise {
                prompt: row.get(0)?,
                answer: row.get(1)?,
            })
        })?;

        let mut exercises = Vec::new();
        for row in rows {
            exercises.push(row?);
        }

        Ok(exercises)
    }

    pub fn record_attempt(
        &self,
        session_id: &str,
        attempt: &AttemptRecord<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let connection = self.open()?;
        connection.execute(
            "INSERT INTO exercise_attempts (
                id, session_id, exercise_id, prompt_snapshot, user_answer,
                accepted_answer, is_correct, explanation_snapshot, focus_snapshot,
                grammar_rule_id, attempt_index, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                generate_id("attempt"),
                session_id,
                attempt.exercise_id,
                attempt.prompt,
                attempt.user_answer,
                attempt.accepted_answer,
                if attempt.is_correct { 1 } else { 0 },
                attempt.explanation,
                attempt.focus,
                attempt.grammar_rule_id,
                attempt.attempt_index as i64,
                unix_timestamp(),
            ],
        )?;

        Ok(())
    }

    fn initialize(&self) -> Result<(), Box<dyn Error>> {
        let connection = self.open()?;
        connection.execute_batch(SCHEMA)?;
        self.run_migrations(&connection)?;
        self.seed_grammar_rules(&connection)?;
        Ok(())
    }

    fn open(&self) -> Result<Connection, Box<dyn Error>> {
        let connection = Connection::open(&self.db_path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
    }

    fn insert_or_get_exercise(
        &self,
        connection: &Connection,
        language: &str,
        level: &str,
        exercise: &Exercise,
    ) -> Result<String, Box<dyn Error>> {
        let existing_id = connection
            .query_row(
                "SELECT id FROM exercises
                 WHERE language = ?1 AND cefr_level = ?2 AND prompt = ?3 AND answer = ?4",
                params![language, level, exercise.sentence, exercise.answer],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(id) = existing_id {
            return Ok(id);
        }

        let exercise_id = generate_id("exercise");
        connection.execute(
            "INSERT INTO exercises (
                id, language, cefr_level, grammar_rule_id, exercise_type, prompt, answer, hint,
                focus, explanation, source, created_at
            ) VALUES (?1, ?2, ?3, ?4, 'cloze', ?5, ?6, ?7, ?8, ?9, 'generated', ?10)",
            params![
                exercise_id,
                language,
                level,
                exercise.focus_rule_id,
                exercise.sentence,
                exercise.answer,
                exercise.hint,
                exercise.focus,
                exercise.explanation,
                unix_timestamp(),
            ],
        )?;

        Ok(exercise_id)
    }

    fn run_migrations(&self, connection: &Connection) -> Result<(), Box<dyn Error>> {
        if !column_exists(connection, "exercises", "grammar_rule_id")? {
            connection.execute(
                "ALTER TABLE exercises ADD COLUMN grammar_rule_id TEXT REFERENCES grammar_rules(id) ON DELETE SET NULL",
                [],
            )?;
        }

        if !column_exists(connection, "exercise_attempts", "grammar_rule_id")? {
            connection.execute(
                "ALTER TABLE exercise_attempts ADD COLUMN grammar_rule_id TEXT REFERENCES grammar_rules(id) ON DELETE SET NULL",
                [],
            )?;
        }

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_exercises_rule ON exercises(grammar_rule_id)",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_attempts_rule ON exercise_attempts(grammar_rule_id, is_correct)",
            [],
        )?;

        Ok(())
    }

    fn seed_grammar_rules(&self, connection: &Connection) -> Result<(), Box<dyn Error>> {
        let rules: Vec<GrammarRule> = serde_json::from_str(SPANISH_GRAMMAR_RULES)?;
        for rule in rules {
            let created_at = unix_timestamp();
            connection.execute(
                "INSERT INTO grammar_rules (
                    id, language, cefr_level, category, title, rule_text,
                    examples_json, common_mistakes_json, tags_json, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                ON CONFLICT(id) DO UPDATE SET
                    language = excluded.language,
                    cefr_level = excluded.cefr_level,
                    category = excluded.category,
                    title = excluded.title,
                    rule_text = excluded.rule_text,
                    examples_json = excluded.examples_json,
                    common_mistakes_json = excluded.common_mistakes_json,
                    tags_json = excluded.tags_json,
                    updated_at = excluded.updated_at",
                params![
                    rule.id,
                    rule.language,
                    rule.cefr_level,
                    rule.category,
                    rule.title,
                    rule.rule_text,
                    serde_json::to_string(&rule.examples)?,
                    serde_json::to_string(&rule.common_mistakes)?,
                    serde_json::to_string(&rule.tags)?,
                    created_at,
                ],
            )?;
        }

        Ok(())
    }
}

fn column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, Box<dyn Error>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;

    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn ensure_parent_dir(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn generate_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
