CREATE TABLE IF NOT EXISTS grammar_rules (
    id TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    cefr_level TEXT,
    category TEXT NOT NULL,
    title TEXT NOT NULL,
    rule_text TEXT NOT NULL,
    examples_json TEXT NOT NULL DEFAULT '[]',
    common_mistakes_json TEXT NOT NULL DEFAULT '[]',
    tags_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS study_sessions (
    id TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    cefr_level TEXT NOT NULL,
    session_type TEXT NOT NULL,
    source_model TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS exercises (
    id TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    cefr_level TEXT NOT NULL,
    grammar_rule_id TEXT,
    exercise_type TEXT NOT NULL,
    prompt TEXT NOT NULL,
    answer TEXT NOT NULL,
    hint TEXT NOT NULL,
    focus TEXT NOT NULL,
    explanation TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(language, cefr_level, prompt, answer),
    FOREIGN KEY (grammar_rule_id) REFERENCES grammar_rules(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS session_exercises (
    session_id TEXT NOT NULL,
    exercise_id TEXT NOT NULL,
    PRIMARY KEY (session_id, exercise_id),
    FOREIGN KEY (session_id) REFERENCES study_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS exercise_attempts (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    exercise_id TEXT NOT NULL,
    grammar_rule_id TEXT,
    prompt_snapshot TEXT NOT NULL,
    user_answer TEXT NOT NULL,
    accepted_answer TEXT NOT NULL,
    is_correct INTEGER NOT NULL,
    explanation_snapshot TEXT NOT NULL,
    focus_snapshot TEXT NOT NULL,
    attempt_index INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (session_id) REFERENCES study_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE,
    FOREIGN KEY (grammar_rule_id) REFERENCES grammar_rules(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_exercises_focus
    ON exercises(language, cefr_level, focus);

CREATE INDEX IF NOT EXISTS idx_exercises_rule
    ON exercises(grammar_rule_id);

CREATE INDEX IF NOT EXISTS idx_attempts_session
    ON exercise_attempts(session_id, attempt_index);

CREATE INDEX IF NOT EXISTS idx_attempts_focus
    ON exercise_attempts(focus_snapshot, is_correct);

CREATE INDEX IF NOT EXISTS idx_attempts_rule
    ON exercise_attempts(grammar_rule_id, is_correct);
