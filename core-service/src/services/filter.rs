use regex::Regex;
use crate::models::FilterRule;

pub struct FilterEngine {
    rules: Vec<FilterRule>,
}

impl FilterEngine {
    pub fn new(rules: Vec<FilterRule>) -> Self {
        Self { rules }
    }

    /// Apply filter rules to determine if content should be included
    pub fn should_include(&self, text: &str) -> bool {
        if self.rules.is_empty() {
            return true;
        }

        let mut included = true;

        for rule in &self.rules {
            if let Ok(regex) = Regex::new(&rule.regex_pattern) {
                let matches = regex.is_match(text);

                match rule.rule_type.as_str() {
                    "Positive" => {
                        // Positive rule: must match
                        included = included && matches;
                    }
                    "Negative" => {
                        // Negative rule: must not match
                        included = included && !matches;
                    }
                    _ => {
                        tracing::warn!("Unknown rule type: {}", rule.rule_type);
                    }
                }
            } else {
                tracing::warn!("Invalid regex pattern: {}", rule.regex_pattern);
            }
        }

        included
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_rule(id: i32, rule_type: &str, pattern: &str) -> FilterRule {
        FilterRule {
            rule_id: id,
            series_id: 1,
            group_id: 1,
            rule_order: id,
            rule_type: rule_type.to_string(),
            regex_pattern: pattern.to_string(),
            created_at: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn test_positive_filter() {
        let rule = create_rule(1, "Positive", "1080p");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("anime 1080p"));
        assert!(!engine.should_include("anime 720p"));
    }

    #[test]
    fn test_negative_filter() {
        let rule = create_rule(1, "Negative", "trash");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("good quality"));
        assert!(!engine.should_include("trash quality"));
    }

    #[test]
    fn test_combined_filters() {
        let rules = vec![
            create_rule(1, "Positive", "1080p|720p"),
            create_rule(2, "Negative", "trash"),
        ];

        let engine = FilterEngine::new(rules);

        assert!(engine.should_include("anime 1080p good"));
        assert!(!engine.should_include("anime 1080p trash"));
        assert!(!engine.should_include("anime 480p"));
    }

    #[test]
    fn test_no_rules() {
        let engine = FilterEngine::new(vec![]);
        assert!(engine.should_include("anything goes"));
    }

    #[test]
    fn test_invalid_regex() {
        let rule = create_rule(1, "Positive", "[invalid");
        let engine = FilterEngine::new(vec![rule]);

        // Invalid regex should still allow content to pass (graceful degradation)
        assert!(engine.should_include("test"));
    }

    #[test]
    fn test_case_sensitive() {
        let rule = create_rule(1, "Positive", "1080p");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("1080p"));
        assert!(!engine.should_include("1080P")); // Case sensitive by default
    }
}
